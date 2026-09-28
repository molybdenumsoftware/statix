use std::{
    default::Default,
    fmt, fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{LintMap, dirs, err::ConfigErr, utils};

use clap::Parser;
use lib::LINTS;
use serde::{Deserialize, Serialize};
use vfs::ReadOnlyVfs;

#[derive(Parser, Debug)]
#[clap(author, about)]
pub struct Opts {
    #[clap(subcommand)]
    pub cmd: SubCommand,
}

#[derive(Parser, Debug)]
pub enum SubCommand {
    /// Lints and suggestions for the nix programming language
    Check(Check),
    /// Find and fix issues raised by statix-check
    Fix(Fix),
    /// Fix exactly one issue at provided position
    Single(Single),
    /// Print detailed explanation for a lint warning
    Explain(Explain),
    /// Dump a sample config to stdout
    Dump(Dump),
    /// List all available lints
    List(List),
}

#[derive(Parser, Debug)]
#[allow(clippy::struct_excessive_bools)] // command line flags
pub struct Check {
    /// Files or directories to run check on. For script files (`.sh`,
    /// `.py`...), the `.nix` files under `.` that refer to them are checked.
    #[clap(default_value = ".")]
    targets: Vec<PathBuf>,

    /// Globs of file patterns to skip
    #[clap(short, long)]
    ignore: Vec<String>,

    /// Don't respect .gitignore files
    #[clap(short, long)]
    unrestricted: bool,

    /// Don't use or update the results cache (`$XDG_CACHE_HOME/statix`)
    #[clap(long)]
    pub no_cache: bool,

    /// Only files changed since REF (default HEAD), including staged,
    /// unstaged and untracked changes. Needs git.
    #[clap(long, value_name = "REF", num_args = 0..=1, default_missing_value = "HEAD")]
    pub changed: Option<String>,

    /// Only staged files. Needs git.
    #[clap(long)]
    pub staged: bool,

    /// Output format.
    /// Supported values: stderr, errfmt, json, agent
    #[clap(short = 'o', long, default_value_t)]
    pub format: OutFormat,

    /// Path to statix.toml or its parent directory
    #[clap(short = 'c', long = "config", default_value = ".")]
    pub conf_path: PathBuf,

    /// Enable "streaming" mode, accept file on stdin, output diagnostics on stdout
    #[clap(short, long = "stdin")]
    pub streaming: bool,
}

impl Check {
    pub fn vfs(
        &self,
        extra_ignores: &[String],
        cache: Option<&crate::cache::Cache>,
    ) -> Result<ReadOnlyVfs, ConfigErr> {
        if self.streaming {
            use std::io::{self, BufRead};
            let src = io::stdin()
                .lock()
                .lines()
                .map(|l| l.unwrap())
                .collect::<Vec<String>>()
                .join("\n");
            Ok(ReadOnlyVfs::singleton("<stdin>", src.as_bytes()))
        } else {
            let all_ignores = [self.ignore.as_slice(), extra_ignores].concat();
            let targets = if self.staged || self.changed.is_some() {
                crate::changed::changed_files(self.changed.as_deref(), self.staged)?
            } else {
                self.targets.clone()
            };
            let files = nix_files_for(&targets, &all_ignores, self.unrestricted, cache)?;
            Ok(vfs(&files))
        }
    }
}

#[derive(Parser, Debug)]
#[allow(clippy::struct_excessive_bools)] // command line flags
pub struct Fix {
    /// Files or directories to run fix on. For script files (`.sh`,
    /// `.py`...), the `.nix` files under `.` that refer to them are fixed.
    #[clap(default_value = ".")]
    targets: Vec<PathBuf>,

    /// Globs of file patterns to skip
    #[clap(short, long)]
    ignore: Vec<String>,

    /// Don't respect .gitignore files
    #[clap(short, long)]
    unrestricted: bool,

    /// Don't use or update the results cache (`$XDG_CACHE_HOME/statix`)
    #[clap(long)]
    pub no_cache: bool,

    /// Only files changed since REF (default HEAD), including staged,
    /// unstaged and untracked changes. Needs git.
    #[clap(long, value_name = "REF", num_args = 0..=1, default_missing_value = "HEAD")]
    pub changed: Option<String>,

    /// Only staged files. Needs git.
    #[clap(long)]
    pub staged: bool,

    /// Do not fix files in place, display a diff instead
    #[clap(short, long = "dry-run")]
    pub diff_only: bool,

    /// Path to statix.toml or its parent directory
    #[clap(short = 'c', long = "config", default_value = ".")]
    pub conf_path: PathBuf,

    /// Enable "streaming" mode, accept file on stdin, output diagnostics on stdout
    #[clap(short, long = "stdin")]
    pub streaming: bool,
}

#[derive(Clone, Copy)]
pub enum FixOut {
    Diff,
    Stream,
    Write,
}

impl Fix {
    pub fn vfs(
        &self,
        extra_ignores: &[String],
        cache: Option<&crate::cache::Cache>,
    ) -> Result<ReadOnlyVfs, ConfigErr> {
        if self.streaming {
            use std::io::{self, BufRead};
            let src = io::stdin()
                .lock()
                .lines()
                .map(|l| l.unwrap())
                .collect::<Vec<String>>()
                .join("\n");
            Ok(ReadOnlyVfs::singleton("<stdin>", src.as_bytes()))
        } else {
            let all_ignores = [self.ignore.as_slice(), extra_ignores].concat();
            let targets = if self.staged || self.changed.is_some() {
                crate::changed::changed_files(self.changed.as_deref(), self.staged)?
            } else {
                self.targets.clone()
            };
            let files = nix_files_for(&targets, &all_ignores, self.unrestricted, cache)?;
            Ok(vfs(&files))
        }
    }

    // i need this ugly helper because clap's data model
    // does not reflect what i have in mind
    #[must_use]
    pub fn out(&self) -> FixOut {
        if self.diff_only {
            FixOut::Diff
        } else if self.streaming {
            FixOut::Stream
        } else {
            FixOut::Write
        }
    }
}

#[derive(Parser, Debug)]
pub struct Single {
    /// File to run single-fix on
    pub target: Option<PathBuf>,

    /// Position to attempt a fix at
    #[clap(short, long)]
    pub position: Position,

    /// Do not fix files in place, display a diff instead
    #[clap(short, long = "dry-run")]
    pub diff_only: bool,

    /// Enable "streaming" mode, accept file on stdin, output diagnostics on stdout
    #[clap(short, long = "stdin")]
    pub streaming: bool,

    /// Path to statix.toml or its parent directory
    #[clap(short = 'c', long = "config", default_value = ".")]
    pub conf_path: PathBuf,
}

impl Single {
    pub fn vfs(&self) -> Result<ReadOnlyVfs, ConfigErr> {
        if self.streaming {
            use std::io::{self, BufRead};
            let src = io::stdin()
                .lock()
                .lines()
                .map(|l| l.unwrap())
                .collect::<Vec<String>>()
                .join("\n");
            Ok(ReadOnlyVfs::singleton("<stdin>", src.as_bytes()))
        } else {
            let src = std::fs::read_to_string(self.target.as_ref().unwrap())
                .map_err(ConfigErr::InvalidPath)?;
            Ok(ReadOnlyVfs::singleton("<stdin>", src.as_bytes()))
        }
    }
    #[must_use]
    pub fn out(&self) -> FixOut {
        if self.diff_only {
            FixOut::Diff
        } else if self.streaming {
            FixOut::Stream
        } else {
            FixOut::Write
        }
    }
}

#[derive(Parser, Debug)]
pub struct Explain {
    /// Warning code to explain
    pub target: WarningCode,
}

#[derive(Parser, Debug)]
pub struct Dump {}

#[derive(Parser, Debug)]
pub struct List {}

#[derive(Debug, Copy, Clone, Default)]
pub enum OutFormat {
    Json,
    Errfmt,
    /// Markdown for coding agents
    Agent,
    #[default]
    StdErr,
}

impl fmt::Display for OutFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Json => "json",
                Self::Errfmt => "errfmt",
                Self::Agent => "agent",
                Self::StdErr => "stderr",
            }
        )
    }
}

impl FromStr for OutFormat {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "errfmt" => Ok(Self::Errfmt),
            "agent" => Ok(Self::Agent),
            "stderr" => Ok(Self::StdErr),
            _ => Err("unknown output format, try: json, errfmt, agent"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ConfFile {
    #[serde(default = "Vec::new")]
    disabled: Vec<String>,

    #[serde(default = "Vec::new")]
    pub ignore: Vec<String>,
}

impl ConfFile {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, ConfigErr> {
        let path = path.as_ref();
        let config_file = fs::read_to_string(path).map_err(ConfigErr::InvalidPath)?;
        toml::de::from_str(&config_file).map_err(ConfigErr::ConfFileParse)
    }
    pub fn discover<P: AsRef<Path>>(path: P) -> Result<Self, ConfigErr> {
        let cannonical_path = fs::canonicalize(path.as_ref()).map_err(ConfigErr::InvalidPath)?;
        for p in cannonical_path.ancestors() {
            let statix_toml_path = if p.is_dir() {
                p.join("statix.toml")
            } else {
                p.to_path_buf()
            };
            if statix_toml_path.exists() {
                return Self::from_path(statix_toml_path);
            }
        }
        Ok(Self::default())
    }
    #[must_use]
    pub fn dump(&self) -> String {
        let ideal_config = {
            let disabled = vec![];
            let ignore = vec![".direnv".into()];
            Self { disabled, ignore }
        };
        toml::ser::to_string_pretty(&ideal_config).unwrap()
    }
    #[must_use]
    pub fn lints(&self) -> LintMap {
        utils::lint_map_of(
            (*LINTS)
                .iter()
                .filter(|l| !self.disabled.iter().any(|check| check == l.name()))
                .copied()
                .collect::<Vec<_>>()
                .as_slice(),
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

impl FromStr for Position {
    type Err = ConfigErr;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let parts = src.split(',');
        match parts.collect::<Vec<_>>().as_slice() {
            [line, col] => {
                let do_parse = |val: &str| {
                    val.parse::<usize>()
                        .map_err(|_| ConfigErr::InvalidPosition(src.to_owned()))
                };
                let l = do_parse(line)?;
                let c = do_parse(col)?;
                Ok(Self { line: l, col: c })
            }
            _ => Err(ConfigErr::InvalidPosition(src.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WarningCode(pub u32);

impl FromStr for WarningCode {
    type Err = ConfigErr;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let mut char_stream = src.chars();
        let severity = char_stream
            .next()
            .ok_or_else(|| ConfigErr::InvalidWarningCode(src.to_owned()))?
            .to_ascii_lowercase();
        match severity {
            'w' => char_stream
                .collect::<String>()
                .parse::<u32>()
                .map(Self)
                .map_err(|_| ConfigErr::InvalidWarningCode(src.to_owned())),
            _ => Ok(Self(0)),
        }
    }
}

/// The `.nix` files to process for `targets`: `.nix` files and directories as
/// given, and for script files (e.g. changed files passed by a git hook) the
/// `.nix` files under `.` that refer to them. `cache` knows what unchanged
/// `.nix` files refer to, so they don't need to be read.
fn nix_files_for(
    targets: &[PathBuf],
    ignores: &[String],
    unrestricted: bool,
    cache: Option<&crate::cache::Cache>,
) -> Result<Vec<PathBuf>, ConfigErr> {
    use rayon::prelude::*;
    let mut files = Vec::new();
    let mut scripts = Vec::new();
    for target in targets {
        dirs::check_path_exists(target)?;
        if target.is_file() && target.extension().is_none_or(|e| e != "nix") {
            scripts.extend(fs::canonicalize(target));
        } else {
            files.extend(dirs::walk_nix_files(target, ignores, unrestricted)?);
        }
    }
    if !scripts.is_empty() {
        let candidates: Vec<PathBuf> = dirs::walk_nix_files(".", ignores, unrestricted)?.collect();
        files.par_extend(candidates.into_par_iter().filter(|nix| {
            let refs = cache.and_then(|c| c.refs_if_unchanged(nix)).or_else(|| {
                let src = fs::read_to_string(nix).ok()?;
                Some(
                    lib::referenced_files(&src, nix)
                        .into_iter()
                        .map(|(path, _, _)| path)
                        .collect(),
                )
            });
            refs.unwrap_or_default()
                .iter()
                .filter_map(|path| fs::canonicalize(path).ok())
                .any(|path| scripts.contains(&path))
        }));
    }
    if targets.len() > 1 || !scripts.is_empty() {
        // `./a.nix` and `a.nix` are the same file
        let normalized = |p: &Path| -> PathBuf {
            p.components()
                .filter(|c| !matches!(c, std::path::Component::CurDir))
                .collect()
        };
        let mut seen = std::collections::HashSet::new();
        files.retain(|f| seen.insert(normalized(f)));
    }
    Ok(files)
}

fn vfs(files: &[PathBuf]) -> vfs::ReadOnlyVfs {
    use rayon::prelude::*;
    let contents: Vec<_> = files.par_iter().map(fs::read_to_string).collect();
    let mut vfs = ReadOnlyVfs::default();
    for (file, data) in files.iter().zip(contents) {
        if let Ok(data) = data {
            let _id = vfs.alloc_file_id(file);
            vfs.set_file_contents(file, data.as_bytes());
        } else {
            println!("`{}` contains non-utf8 content", file.display());
        }
    }
    vfs
}
