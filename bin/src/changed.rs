//! Files changed according to git, for `--changed` and `--staged`.

use std::{path::PathBuf, process::Command};

/// Files that can affect Nix lint results: `.nix` and script files.
const EXTENSIONS: &[&str] = &["nix", "sh", "bash", "py"];

fn git(args: &[&str]) -> std::io::Result<Vec<PathBuf>> {
    let output = Command::new("git").args(args).output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(output
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| PathBuf::from(String::from_utf8_lossy(p).as_ref()))
        .collect())
}

/// Existing files changed since `base` (committed, staged, unstaged or
/// untracked), or only the staged ones.
pub fn changed_files(base: Option<&str>, staged: bool) -> std::io::Result<Vec<PathBuf>> {
    // paths relative to the current directory, like the other targets
    let mut files = if staged {
        git(&[
            "diff",
            "--cached",
            "--name-only",
            "-z",
            "--relative",
            "--diff-filter=ACMR",
        ])?
    } else {
        let base = base.unwrap_or("HEAD");
        let mut files = git(&[
            "diff",
            "--name-only",
            "-z",
            "--relative",
            "--diff-filter=ACMR",
            base,
        ])?;
        files.extend(git(&["ls-files", "--others", "--exclude-standard", "-z"])?);
        files
    };
    files.retain(|p| {
        p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| EXTENSIONS.contains(&e))
    });
    files.sort();
    files.dedup();
    Ok(files)
}
