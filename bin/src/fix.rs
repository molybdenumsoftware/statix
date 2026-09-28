use std::borrow::Cow;

use crate::LintMap;

use rnix::TextRange;

mod all;
use all::{MAX_PASSES, pass};

mod single;
use single::single;

type Source<'a> = Cow<'a, str>;

pub struct FixResult<'a> {
    pub src: Source<'a>,
    pub fixed: Vec<Fixed>,
    pub lints: &'a LintMap,
}

#[derive(Debug, Clone)]
pub struct Fixed {
    pub at: TextRange,
    pub code: u32,
}

impl<'a> FixResult<'a> {
    fn empty(src: Source<'a>, lints: &'a LintMap) -> Self {
        Self {
            src,
            fixed: Vec::new(),
            lints,
        }
    }
}

pub mod main {
    use std::borrow::Cow;

    use crate::{
        config::{
            FixOut, Single as SingleConfig, {ConfFile, Fix as FixConfig},
        },
        err::{FixErr, StatixErr},
    };

    use similar::TextDiff;

    fn print_diff(path: &std::path::Path, old: &str, new: &str) {
        let text_diff = TextDiff::from_lines(old, new);
        let old_file = format!("{}", path.display());
        let new_file = format!("{} [fixed]", path.display());
        println!(
            "{}",
            text_diff
                .unified_diff()
                .context_radius(4)
                .header(&old_file, &new_file)
        );
    }

    /// A fixed Nix file and the script files it refers to.
    struct Outcome {
        path: std::path::PathBuf,
        original: String,
        fixed: Option<String>,
        scripts: Vec<(std::path::PathBuf, lib::Lang, lib::Kind)>,
    }

    pub fn all(fix_config: &FixConfig) -> Result<(), StatixErr> {
        use rayon::prelude::*;

        let conf_file = ConfFile::discover(&fix_config.conf_path)?;
        let vfs = fix_config.vfs(conf_file.ignore.as_slice())?;
        let out = fix_config.out();

        let lints = conf_file.lints();
        let fix_scripts = lints.values().flatten().any(|l| l.name() == "script_file");
        // `script_file` only reports; referenced scripts are fixed below.
        let mut nix_lints = lints.clone();
        for rules in nix_lints.values_mut() {
            rules.retain(|l| l.name() != "script_file");
        }

        // 1. Nix files: all files do one fix pass in parallel, then every
        // script the pass produced is checked in batches across files, and
        // again until no file changes.
        let entries: Vec<_> = vfs.iter().collect();
        let mut current: Vec<Option<String>> = vec![None; entries.len()];
        let mut active: Vec<usize> = (0..entries.len()).collect();
        let check_scripts = lints.values().flatten().any(|l| l.name() == "shellcheck");
        for _ in 0..super::MAX_PASSES {
            if active.is_empty() {
                break;
            }
            if check_scripts {
                let sources: Vec<(&std::path::Path, &str)> = active
                    .iter()
                    .map(|&i| {
                        let src = current[i].as_deref().unwrap_or(entries[i].contents);
                        (entries[i].file_path, src)
                    })
                    .collect();
                crate::lint::prefetch(&sources);
            }
            let results: Vec<(usize, Option<String>)> = active
                .par_iter()
                .map(|&i| {
                    let src = current[i].as_deref().unwrap_or(entries[i].contents);
                    let next = lib::with_current_file(Some(entries[i].file_path), || {
                        super::pass(src, &nix_lints)
                    });
                    (i, next)
                })
                .collect();
            active.clear();
            for (i, next) in results {
                if let Some(next) = next {
                    current[i] = Some(next);
                    active.push(i);
                }
            }
        }

        let mut fixed: Vec<Outcome> = entries
            .par_iter()
            .zip(current)
            .map(|(entry, fixed)| -> Result<Outcome, FixErr> {
                if let (FixOut::Write, Some(src)) = (out, &fixed) {
                    std::fs::write(entry.file_path, src).map_err(FixErr::InvalidPath)?;
                }
                let src = fixed.as_deref().unwrap_or(entry.contents);
                let scripts = if fix_scripts && !matches!(out, FixOut::Stream) {
                    lib::referenced_files(src, entry.file_path)
                } else {
                    Vec::new()
                };
                Ok(Outcome {
                    path: entry.file_path.to_path_buf(),
                    original: entry.contents.to_string(),
                    fixed,
                    scripts,
                })
            })
            .collect::<Result<_, _>>()?;
        fixed.sort_by(|a, b| a.path.cmp(&b.path));
        for f in &fixed {
            let src = f.fixed.as_deref().unwrap_or(&f.original);
            match out {
                FixOut::Diff => print_diff(&f.path, &f.original, src),
                FixOut::Stream => println!("{src}"),
                FixOut::Write => (),
            }
        }

        // 2. Script files they refer to, each once, in parallel
        let mut scripts: Vec<_> = fixed.into_iter().flat_map(|f| f.scripts).collect();
        scripts.sort_by(|a, b| a.0.cmp(&b.0));
        scripts.dedup_by(|a, b| a.0 == b.0);
        let diffs: Vec<_> = scripts
            .par_iter()
            .map(|(path, lang, kind)| -> Result<_, FixErr> {
                let Ok(old) = std::fs::read_to_string(path) else {
                    return Ok(None);
                };
                let Some(new) = lib::fix_text(*lang, *kind, &old, Some(path)) else {
                    return Ok(None);
                };
                if matches!(out, FixOut::Write) {
                    std::fs::write(path, &new).map_err(FixErr::InvalidPath)?;
                }
                Ok(Some((path, old, new)))
            })
            .collect::<Result<_, _>>()?;
        if matches!(out, FixOut::Diff) {
            for (path, old, new) in diffs.into_iter().flatten() {
                print_diff(path, &old, &new);
            }
        }
        Ok(())
    }

    pub fn single(single_config: &SingleConfig) -> Result<(), StatixErr> {
        let vfs = single_config.vfs()?;
        let entry = vfs.iter().next().unwrap();
        let path = entry.file_path.display().to_string();
        let original_src = entry.contents;

        match (
            single_config.out(),
            lib::with_current_file(Some(entry.file_path), || {
                super::single(single_config.position, original_src)
            }),
        ) {
            (FixOut::Diff, single_result) => {
                let fixed_src = single_result
                    .map(|r| r.src)
                    .unwrap_or(Cow::Borrowed(original_src));
                let text_diff = TextDiff::from_lines(original_src, &fixed_src);
                let old_file = &path;
                let new_file = format!("{path} [fixed]");
                println!(
                    "{}",
                    text_diff
                        .unified_diff()
                        .context_radius(4)
                        .header(old_file, &new_file)
                );
            }
            (FixOut::Stream, single_result) => {
                let src = single_result
                    .map(|r| r.src)
                    .unwrap_or(Cow::Borrowed(original_src));
                println!("{src}");
            }
            (FixOut::Write, Ok(single_result)) => {
                let path = entry.file_path;
                std::fs::write(path, &*single_result.src).map_err(FixErr::InvalidPath)?;
            }
            (_, Err(e)) => return Err(e.into()),
        }
        Ok(())
    }
}
