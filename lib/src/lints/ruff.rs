use crate::{Metadata, Report, Rule, scripts};

use macros::lint;
use rnix::{NodeOrToken, SyntaxElement, SyntaxKind, ast};
use rowan::ast::AstNode as _;

/// ## What it does
/// Runs [ruff](https://docs.astral.sh/ruff) on Python scripts written as Nix
/// strings and reports its findings at their position in the `.nix` file:
/// `writers.writePython3(Bin)`, `writePyPy3(Bin)`, `writeScript(Bin)` with a
/// Python shebang, and devenv scripts/tasks with a Python `package`.
///
/// `${...}` interpolations are replaced by a placeholder name. `statix fix`
/// applies ruff's safe fixes. Uses the project's ruff configuration. Needs
/// `ruff` in `PATH` (or `STATIX_RUFF` pointing to it); without it this lint
/// does nothing. Python files referenced from Nix are checked by
/// `script_file`.
///
/// ## Why is this bad?
/// Nix doesn't check the scripts it writes, so unused imports, undefined
/// names and syntax errors only show up when the script runs.
///
/// ## Example
///
/// ```nix
/// pkgs.writers.writePython3Bin "hello" { } ''
///   import os
///   print("hello")
/// ''
/// ```
///
/// Remove the unused import:
///
/// ```nix
/// pkgs.writers.writePython3Bin "hello" { } ''
///   print("hello")
/// ''
/// ```
#[lint(
    name = "ruff",
    note = "ruff found issues in an inline Python script",
    code = 29,
    match_with = SyntaxKind::NODE_STRING
)]
struct Ruff;

impl Rule for Ruff {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let s = ast::Str::cast(node.clone())?;
        let checked = scripts::check_string(&s, true)?;
        scripts::string_report(self.report(), &s, &checked)
    }
}
