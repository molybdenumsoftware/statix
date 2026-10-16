use crate::{Metadata, Report, Rule, scripts};

use macros::lint;
use rnix::{NodeOrToken, SyntaxElement, SyntaxKind, ast};
use rowan::ast::AstNode as _;

/// ## What it does
/// Runs [ShellCheck](https://www.shellcheck.net) on shell scripts written as
/// Nix strings and reports its findings at their position in the `.nix` file:
///
/// - devenv: `scripts.<name>.exec`, `tasks.<name>.exec`/`.status`,
///   `processes.<name>.exec`, `enterShell`, `enterTest`
/// - nixpkgs: `writeShellScript(Bin)`, `writeShellApplication { text }`,
///   `writers.writeBash(Bin)`, `writers.writeDash(Bin)`, and
///   `writeScript(Bin)` with a shell shebang
/// - stdenv phases and hooks (`buildPhase`, `postInstall`, `shellHook`...),
///   `runCommand` scripts
/// - NixOS `systemd.services.<name>.script`/`preStart`/... and
///   `system.activationScripts`
/// - `let` bindings used as one of the above, or interpolated into one
///   (`${snippet}`), are checked as script fragments
///
/// Script files referenced from Nix are checked by `script_file`.
///
/// `${...}` interpolations are replaced by a placeholder word; findings that
/// only exist because of the placeholder are dropped. `statix fix` applies
/// shellcheck's own fixes plus built-in ones for SC2045, SC2115, SC2155 and
/// SC2162. Other findings carry a `help` text (see `--format agent`).
///
/// Needs `shellcheck` in `PATH` (or `STATIX_SHELLCHECK` pointing to it);
/// without it this lint does nothing.
///
/// ## Why is this bad?
/// Nix doesn't check the scripts it writes, so quoting bugs and typos only
/// show up when the script runs.
///
/// ## Example
///
/// ```nix
/// {
///   scripts.hello.exec = ''
///     echo $1
///   '';
/// }
/// ```
///
/// Quote the variable:
///
/// ```nix
/// {
///   scripts.hello.exec = ''
///     echo "$1"
///   '';
/// }
/// ```
#[lint(
    name = "shellcheck",
    note = "ShellCheck found issues in an inline shell script",
    code = 28,
    match_with = SyntaxKind::NODE_STRING
)]
struct ShellCheck;

impl Rule for ShellCheck {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let s = ast::Str::cast(node.clone())?;
        let checked = scripts::check_string(&s, false)?;
        scripts::string_report(self.report(), &s, &checked)
    }
}
