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
        config::{Check as CheckConfig, ConfFile},
        err::StatixErr,
        traits::WriteDiagnostic,
    };

    use rayon::prelude::*;

    pub fn main(check_config: &CheckConfig) -> Result<(), StatixErr> {
        let conf_file = ConfFile::discover(&check_config.conf_path)?;
        let lints = conf_file.lints();

        let vfs = check_config.vfs(conf_file.ignore.as_slice())?;

        let mut stdout = io::stdout();
        let sources: Vec<_> = vfs.iter().map(|e| (e.file_path, e.contents)).collect();
        prefetch(&sources);
        let lint = |vfs_entry| lint_with(&vfs_entry, &lints);
        let results = vfs
            .par_iter()
            .map(lint)
            .filter(|lr| !lr.reports.is_empty())
            .collect::<Vec<_>>();

        if !results.is_empty() {
            for r in &results {
                stdout.write(r, &vfs, check_config.format).unwrap();
            }
            std::process::exit(1);
        }

        std::process::exit(0);
    }
}
