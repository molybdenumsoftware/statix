//! Running shellcheck and ruff, normalized into [`Finding`]s.

use super::{Finding, Level, fixes};
use serde::Deserialize;
use std::{
    ffi::OsStr,
    io::Write as _,
    path::Path,
    process::{Command, Stdio},
};

/// Run `program` (overridable with `env`) with `script` on stdin.
fn run(env: &str, program: &str, args: &[&OsStr], script: &str) -> Option<Vec<u8>> {
    let program = std::env::var_os(env).unwrap_or_else(|| program.into());
    let mut child = Command::new(program)
        .args(args)
        // statix already runs one checker per core
        .env("RAYON_NUM_THREADS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.take()?.write_all(script.as_bytes()).ok()?;
    Some(child.wait_with_output().ok()?.stdout)
}

#[derive(Deserialize)]
struct ScOutput {
    comments: Vec<ScComment>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScComment {
    #[serde(default)]
    file: String,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
    level: String,
    code: u32,
    message: String,
    fix: Option<ScFix>,
}

#[derive(Deserialize)]
struct ScFix {
    replacements: Vec<fixes::Change>,
}

fn shellcheck_finding(c: ScComment, script: &str) -> Finding {
    Finding {
        fix: c
            .fix
            .map(|f| f.replacements)
            .or_else(|| fixes::builtin(c.code, script, c.line, c.column, c.end_line, c.end_column)),
        line: c.line,
        column: c.column,
        end_line: c.end_line,
        end_column: c.end_column,
        level: match c.level.as_str() {
            "error" => Level::Error,
            "warning" => Level::Warning,
            _ => Level::Info,
        },
        code: format!("SC{}", c.code),
        message: c.message,
        url: Some(format!("https://www.shellcheck.net/wiki/SC{}", c.code)),
    }
}

const SHELLCHECK_ARGS: [&str; 3] = ["--format=json1", "--exclude=SC1091", "--shell"];

pub fn shellcheck(shell: &str, script: &str) -> Option<Vec<Finding>> {
    let mut args: Vec<&OsStr> = SHELLCHECK_ARGS.iter().map(OsStr::new).collect();
    args.extend([OsStr::new(shell), OsStr::new("-")]);
    let stdout = run("STATIX_SHELLCHECK", "shellcheck", &args, script)?;
    let output: ScOutput = serde_json::from_slice(&stdout).ok()?;
    Some(
        output
            .comments
            .into_iter()
            .map(|c| shellcheck_finding(c, script))
            .collect(),
    )
}

/// Check many scripts with one shellcheck process: they're written to temp
/// files, and each comment names the file it belongs to. Process startup is
/// most of shellcheck's cost for small scripts.
pub fn shellcheck_many(shell: &str, scripts: &[&str]) -> Option<Vec<Vec<Finding>>> {
    let dir = tempfile::tempdir().ok()?;
    let paths: Vec<std::path::PathBuf> = scripts
        .iter()
        .enumerate()
        .map(|(i, script)| {
            let path = dir.path().join(format!("{i}.sh"));
            std::fs::write(&path, script).ok().map(|()| path)
        })
        .collect::<Option<_>>()?;
    let mut args: Vec<&OsStr> = SHELLCHECK_ARGS.iter().map(OsStr::new).collect();
    args.push(OsStr::new(shell));
    args.extend(paths.iter().map(|p| p.as_os_str()));
    let stdout = run("STATIX_SHELLCHECK", "shellcheck", &args, "")?;
    let output: ScOutput = serde_json::from_slice(&stdout).ok()?;

    let mut out = vec![Vec::new(); scripts.len()];
    for c in output.comments {
        let index = Path::new(&c.file)
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&i| i < scripts.len())?;
        out[index].push(shellcheck_finding(c, scripts[index]));
    }
    Some(out)
}

#[derive(Deserialize)]
struct RuffLocation {
    row: usize,
    column: usize,
}

#[derive(Deserialize)]
struct RuffEdit {
    content: String,
    location: RuffLocation,
    end_location: RuffLocation,
}

#[derive(Deserialize)]
struct RuffFix {
    applicability: String,
    edits: Vec<RuffEdit>,
}

#[derive(Deserialize)]
struct RuffDiagnostic {
    code: Option<String>,
    message: String,
    location: RuffLocation,
    end_location: RuffLocation,
    fix: Option<RuffFix>,
    url: Option<String>,
}

/// `filename` lets ruff find the project's configuration.
pub fn ruff(script: &str, filename: Option<&Path>) -> Option<Vec<Finding>> {
    let name = filename.map_or_else(|| Path::new("inline.py").as_os_str(), Path::as_os_str);
    let args: Vec<&OsStr> = [
        OsStr::new("check"),
        OsStr::new("--output-format=json"),
        OsStr::new("--no-cache"),
        OsStr::new("--stdin-filename"),
        name,
        OsStr::new("-"),
    ]
    .to_vec();
    let stdout = run("STATIX_RUFF", "ruff", &args, script)?;
    let diagnostics: Vec<RuffDiagnostic> = serde_json::from_slice(&stdout).ok()?;
    Some(
        diagnostics
            .into_iter()
            .map(|d| Finding {
                line: d.location.row,
                column: d.location.column,
                end_line: d.end_location.row,
                end_column: d.end_location.column,
                level: match d.code.as_deref() {
                    None | Some("invalid-syntax") => Level::Error,
                    Some(_) => Level::Warning,
                },
                code: d.code.unwrap_or_else(|| "syntax-error".into()),
                message: d.message,
                url: d.url,
                fix: d.fix.filter(|f| f.applicability == "safe").map(|f| {
                    f.edits
                        .into_iter()
                        .map(|e| fixes::Change {
                            line: e.location.row,
                            column: e.location.column,
                            end_line: e.end_location.row,
                            end_column: e.end_location.column,
                            replacement: e.content,
                        })
                        .collect()
                }),
            })
            .collect(),
    )
}

/// The script with ruff's safe fixes applied, by ruff itself (it orders and
/// repeats them). `None` when ruff isn't installed or nothing changes.
pub fn ruff_fix(script: &str, filename: Option<&Path>) -> Option<String> {
    let name = filename.map_or_else(|| Path::new("inline.py").as_os_str(), Path::as_os_str);
    let args: Vec<&OsStr> = [
        OsStr::new("check"),
        OsStr::new("--fix-only"),
        OsStr::new("--exit-zero"),
        OsStr::new("--no-cache"),
        OsStr::new("--quiet"),
        OsStr::new("--stdin-filename"),
        name,
        OsStr::new("-"),
    ]
    .to_vec();
    let fixed = String::from_utf8(run("STATIX_RUFF", "ruff", &args, script)?).ok()?;
    (!fixed.is_empty() && fixed != script).then_some(fixed)
}

/// `--version` of shellcheck and ruff (and which binaries), empty when missing.
pub fn versions() -> String {
    let version = |env: &str, program: &str| {
        let stdout = run(env, program, &[OsStr::new("--version")], "").unwrap_or_default();
        format!(
            "{}={}:{}",
            program,
            std::env::var(env).unwrap_or_default(),
            String::from_utf8_lossy(&stdout).trim()
        )
    };
    format!(
        "{} {}",
        version("STATIX_SHELLCHECK", "shellcheck"),
        version("STATIX_RUFF", "ruff")
    )
}
