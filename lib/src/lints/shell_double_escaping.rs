use crate::{Metadata, Report, Rule, Severity};
use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{InterpolPart, Str},
};
use rowan::ast::AstNode as _;

use super::shell_context::{Quote, escaped_arg, prefix_at, script_parts};

/// ## What it does
/// Advises when a recognized shell script wraps an entire `lib.escapeShellArg`
/// hole (or `lib.escapeShellArgs` applied to a nonempty literal list) in single quotes.
///
/// ## Why is this bad?
/// The outer single quotes can cancel the quoting emitted by the helper, exposing
/// spaces or shell syntax. This failure and its removal are documented by Home
/// Manager's merged fix: <https://github.com/nix-community/home-manager/pull/9408>.
/// A required supplied `lib` is conventional, not evaluated; locally defined
/// values and defaulted parameters are skipped.
/// The cited change uses an inherited unqualified helper in a let-bound callable;
/// that provenance and consumer flow are intentionally outside this baseline.
///
/// This is bounded AST/lexical advice, not full shell analysis. Only whole-word
/// single-quoted helper holes in recognized shell consumers are checked. Unknown
/// earlier interpolation, redirection/heredocs, arithmetic, backticks, complex
/// grammar and nested shell-string consumers are excluded. No fix is offered.
///
/// ## Example
/// ```nix
/// { lib, filePath }: { script = ''cat '${lib.escapeShellArg filePath}' ''; }
/// ```
#[lint(
    name = "shell_double_escaping",
    note = "Shell escape helper wrapped in single quotes",
    code = 33,
    match_with = SyntaxKind::NODE_STRING
)]
struct ShellDoubleEscaping;

impl Rule for ShellDoubleEscaping {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let string = Str::cast(node.clone())?;
        if !string.parts().any(|part| {
            matches!(part, InterpolPart::Interpolation(hole) if hole.expr().is_some_and(escaped_arg))
        }) {
            return None;
        }
        let parts = script_parts(&string)?;
        let mut report = self.report().severity(Severity::Hint);
        for (index, part) in parts.iter().enumerate() {
            let InterpolPart::Interpolation(hole) = part else {
                continue;
            };
            if !hole.expr().is_some_and(escaped_arg) {
                continue;
            }
            let Some(InterpolPart::Literal(before)) =
                index.checked_sub(1).and_then(|i| parts.get(i))
            else {
                continue;
            };
            let Some(InterpolPart::Literal(after)) = parts.get(index + 1) else {
                continue;
            };
            if !before.ends_with('\'') || !after.starts_with('\'') {
                continue;
            }
            // The opening quote must begin a new word, not an adjacent quote
            // segment or a quote embedded in a composed argument.
            let Some(opening) = prefix_at(&parts, index - 1, before.len() - 1, false) else {
                continue;
            };
            if opening.quote != Quote::Unquoted
                || opening.word_active
                || opening.position.is_command()
                || opening.comment
            {
                continue;
            }
            let tail = &after[1..];
            let boundary = tail.as_bytes().first().is_none_or(|byte| {
                matches!(
                    byte,
                    b' ' | b'\t' | b'\r' | b'\n' | b';' | b'|' | b'&' | b')'
                )
            });
            // An immediately following Nix hole composes the same word.
            if !boundary
                || (tail.is_empty()
                    && matches!(parts.get(index + 2), Some(InterpolPart::Interpolation(_))))
            {
                continue;
            }
            report = report.diagnostic(
                hole.syntax().text_range(),
                "Outer single quotes can cancel `lib.escapeShellArg`/`lib.escapeShellArgs` quoting; consider using the helper output as an unquoted shell word",
            );
        }
        (!report.diagnostics.is_empty()).then_some(report)
    }
}
