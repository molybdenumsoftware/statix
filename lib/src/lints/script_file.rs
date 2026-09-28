use crate::{Metadata, Report, Rule, scripts};

use macros::lint;
use rnix::{NodeOrToken, SyntaxElement, SyntaxKind};

/// ## What it does
/// Checks shell and Python script files that Nix code refers to, with
/// `shellcheck` and `ruff`:
///
/// - paths to `.sh`, `.bash` and `.py` files (or files with a shell/Python
///   shebang): `exec = ./deploy.sh;`, `builtins.readFile ./x.sh`,
///   `"${./x.sh}"`, `updateScript = ./update.sh;`, setup hooks
/// - files a devenv script runs by relative path: `bash ./scripts/x.sh`,
///   `python scripts/x.py` (devenv runs scripts from the project root)
///
/// Findings are reported at the reference, with the file's own line and
/// column. `statix fix` applies the checkers' fixes to the referenced file.
///
/// ## Why is this bad?
/// Scripts next to the Nix code are part of the build or dev environment but
/// usually aren't linted with it.
///
/// ## Example
///
/// ```nix
/// {
///   scripts.deploy.exec = ./deploy.sh; # deploy.sh: rm -rf $DIR/*
/// }
/// ```
///
/// Fix the findings in `deploy.sh`, e.g. `rm -rf "${DIR:?}"/*`.
#[lint(
    name = "script_file",
    note = "Script file referenced from Nix has issues",
    code = 30,
    match_with = [SyntaxKind::NODE_PATH_REL, SyntaxKind::NODE_STRING]
)]
struct ScriptFile;

impl Rule for ScriptFile {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let mut report = self.report();
        let mut level = scripts::Level::Info;
        let mut seen = Vec::new();
        for reference in scripts::context::references(node) {
            if seen.contains(&reference.path)
                || scripts::context::referenced_earlier(node, &reference)
            {
                continue;
            }
            seen.push(reference.path.clone());
            let Ok(text) = std::fs::read_to_string(&reference.path) else {
                continue;
            };
            let Some(findings) = scripts::check(
                reference.lang,
                reference.kind,
                &text,
                Some(&reference.path),
                &|_| false,
            ) else {
                continue;
            };
            for f in findings {
                level = level.max(f.level);
                let message = format!(
                    "{}:{}:{}: {}: {}",
                    reference.written, f.line, f.column, f.code, f.message
                );
                let external = crate::External {
                    path: reference.path.clone(),
                    line: f.line,
                    column: f.column,
                };
                report = if f.fix.is_some() {
                    report.external_fixed_later(reference.at, message, external)
                } else {
                    report.external_with_help(reference.at, message, f.help(), external)
                };
            }
        }
        if report.diagnostics.is_empty() {
            return None;
        }
        Some(report.severity(scripts::severity(level)))
    }
}
