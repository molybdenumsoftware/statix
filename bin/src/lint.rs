use crate::LintMap;

use lib::Report;
use rnix::{Root, WalkEvent};
use vfs::{FileId, VfsEntry};

#[derive(Debug)]
pub struct LintResult {
    pub file_id: FileId,
    pub reports: Vec<Report>,
}

#[must_use]
pub fn lint_with(vfs_entry: &VfsEntry, lints: &LintMap) -> LintResult {
    let file_id = vfs_entry.file_id;
    let source = vfs_entry.contents;
    let parsed = Root::parse(source);
    let reports = lib::with_current_file(Some(vfs_entry.file_path), || {
        // one checker process per file instead of one per script
        lib::prefetch(&parsed.syntax());
        lints_of(&parsed, lints)
    });
    LintResult { file_id, reports }
}

fn lints_of(parsed: &rnix::Parse<Root>, lints: &LintMap) -> Vec<Report> {
    let error_reports = parsed.errors().iter().map(Report::from_parse_err);
    parsed
        .syntax()
        .preorder_with_tokens()
        .filter_map(|event| match event {
            WalkEvent::Enter(child) => lints.get(&child.kind()).map(|rules| {
                rules
                    .iter()
                    .filter_map(|rule| rule.validate(&child))
                    .collect::<Vec<_>>()
            }),
            WalkEvent::Leave(_) => None,
        })
        .flatten()
        .chain(error_reports)
        .collect()
}

/// Check all shell scripts of `sources` (path, contents) up front, batched
/// across files: a few hundred checker processes instead of one per file.
pub fn prefetch(sources: &[(&std::path::Path, &str)]) {
    use rayon::prelude::*;
    let scripts = sources
        .par_iter()
        .flat_map_iter(|(path, src)| {
            lib::with_current_file(Some(path), || {
                lib::shell_scripts(&Root::parse(src).syntax())
            })
        })
        .collect();
    lib::prefetch_scripts(scripts);
}

pub mod main {
    use std::io;

    use super::{lint_with, prefetch};
    use crate::{
        cache::Cache,
        config::{Check as CheckConfig, ConfFile},
        err::StatixErr,
        traits::WriteDiagnostic,
    };

    use rayon::prelude::*;

    pub fn main(check_config: &CheckConfig) -> Result<(), StatixErr> {
        let conf_file = ConfFile::discover(&check_config.conf_path)?;
        let lints = conf_file.lints();
        let use_cache = !check_config.no_cache && !check_config.streaming;
        let mut cache = Cache::open(&lints, &conf_file, use_cache);

        let vfs = check_config.vfs(conf_file.ignore.as_slice(), cache.as_ref())?;

        // Files without findings last time, unchanged since, are skipped.
        let entries: Vec<_> = vfs.iter().collect();
        let stale: Vec<_> = entries
            .par_iter()
            .filter(|e| {
                cache
                    .as_ref()
                    .is_none_or(|c| !c.is_clean(e.file_path, e.contents))
            })
            .collect();

        if !stale.is_empty()
            && let Some(cache) = cache.as_mut()
        {
            cache.load_scripts();
        }
        let sources: Vec<_> = stale.iter().map(|e| (e.file_path, e.contents)).collect();
        prefetch(&sources);
        let results: Vec<_> = stale
            .par_iter()
            .map(|entry| {
                let result = lint_with(entry, &lints);
                let refs = cache.is_some().then(|| {
                    lib::referenced_files(entry.contents, entry.file_path)
                        .into_iter()
                        .map(|(path, _, _)| path)
                        .collect::<Vec<_>>()
                });
                (entry, result, refs)
            })
            .collect();

        if let Some(cache) = cache.as_mut() {
            for entry in &entries {
                cache.touch(entry.file_path);
            }
            for (entry, result, refs) in &results {
                let clean = result.reports.is_empty();
                cache.record(
                    entry.file_path,
                    entry.contents,
                    refs.clone().unwrap_or_default(),
                    clean,
                );
            }
        }
        if let Some(cache) = cache {
            cache.save();
        }

        let mut stdout = io::stdout();
        let failed: Vec<_> = results
            .iter()
            .filter(|(_, r, _)| !r.reports.is_empty())
            .collect();
        for (_, r, _) in &failed {
            stdout.write(r, &vfs, check_config.format).unwrap();
        }
        std::process::exit(i32::from(!failed.is_empty()));
    }
}
