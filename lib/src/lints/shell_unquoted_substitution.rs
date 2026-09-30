use crate::{Metadata, Report, Rule, Severity};
use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{AstToken, InterpolPart, Str},
};
use rowan::ast::AstNode as _;

use super::shell_context::{Prefix, Quote, prefix_at, script_parts};

/// ## What it does
/// Advises about a literal, unquoted whole-word `$(dirname ...)` substitution in
/// argument position in a recognized shell script. Only a single simple input
/// argument (optionally preceded by `--`) is recognized.
///
/// ## Why is this bad?
/// Escaping the input does not quote the output: if `dirname` emits whitespace or
/// glob characters, the caller's shell may split or expand the result. Home
/// Manager fixed that distinction by double-quoting the output in its merged
/// change: <https://github.com/nix-community/home-manager/pull/9408>.
/// Its inherited unqualified helper and let-bound callable flow are not resolved
/// by this baseline; the citation establishes the hazard, not corpus coverage.
///
/// This is conditional advice, not proof of a runtime failure or full shell
/// analysis. Unknown prior Nix holes, composed output words, command position,
/// redirection/heredocs, arithmetic, backticks, nested shell-string consumers and
/// complex substitution bodies are excluded. No automatic shell rewrite is made.
///
/// ## Example
/// ```nix
/// { lib, filePath }: { script = ''mkdir -p $(dirname ${lib.escapeShellArg filePath})''; }
/// ```
#[lint(
    name = "shell_unquoted_substitution",
    note = "Unquoted dirname command substitution",
    code = 34,
    match_with = SyntaxKind::NODE_STRING
)]
struct ShellUnquotedSubstitution;

impl Rule for ShellUnquotedSubstitution {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let string = Str::cast(node.clone())?;
        if !string.parts().any(|part| {
            matches!(part, InterpolPart::Literal(text) if text.syntax().text().contains("$("))
        }) {
            return None;
        }
        let parts = script_parts(&string)?;
        for (index, part) in parts.iter().enumerate() {
            let InterpolPart::Literal(text) = part else {
                continue;
            };
            for (offset, _) in text.match_indices("$(dirname") {
                if !text
                    .as_bytes()
                    .get(offset + 9)
                    .is_some_and(u8::is_ascii_whitespace)
                {
                    continue;
                }
                let Some(outer) = prefix_at(&parts, index, offset, true) else {
                    continue;
                };
                if outer.quote != Quote::Unquoted
                    || outer.comment
                    || outer.position.is_command()
                    || outer.word_active
                {
                    continue;
                }
                if simple_dirname(&parts, index, offset + 9, &outer) {
                    // rnix-normalized offsets need not match raw Nix escape or
                    // indentation offsets; deliberately locate the original AST
                    // string instead of inventing a source mapping.
                    return Some(self.report().severity(Severity::Hint).diagnostic(
                        node.text_range(),
                        "If `dirname` outputs whitespace or glob characters, this unquoted substitution may split or expand; consider double-quoting the output even when its input is escaped",
                    ));
                }
            }
        }
        None
    }
}

fn simple_dirname(
    parts: &[InterpolPart<String>],
    start_index: usize,
    start_offset: usize,
    outer: &Prefix<'_>,
) -> bool {
    let depth = outer.substitution_depth + 1;
    for (index, part) in parts.iter().enumerate().skip(start_index) {
        let InterpolPart::Literal(text) = part else {
            continue;
        };
        let from = if index == start_index {
            start_offset
        } else {
            0
        };
        for (relative, _) in text[from..].match_indices(')') {
            let offset = from + relative;
            let Some(inside) = prefix_at(parts, index, offset, true) else {
                return false;
            };
            if inside.quote != Quote::Unquoted || inside.substitution_depth != depth {
                continue;
            }
            if inside.comment || inside.position.is_command() || inside.position.is_statement() {
                return false;
            }
            let tail = &text[offset + 1..];
            if !tail.as_bytes().first().is_none_or(|byte| {
                matches!(
                    byte,
                    b' ' | b'\t' | b'\r' | b'\n' | b';' | b'|' | b'&' | b')'
                )
            }) || (tail.is_empty()
                && matches!(parts.get(index + 1), Some(InterpolPart::Interpolation(_))))
            {
                return false;
            }
            let Some(words) = inside.words.get(outer.words.len()..) else {
                return false;
            };
            let Some(command) = words.first() else {
                return false;
            };
            if command.text != Some("dirname")
                || !command.command_start
                || command.quoted
                || words.iter().any(|word| word.substitution_depth != depth)
                || words.iter().skip(1).any(|word| word.command_start)
            {
                return false;
            }
            let arguments = &words[1..];
            let count = arguments.len() + usize::from(inside.word_active);
            let dash_dash = arguments
                .first()
                .is_some_and(|word| word.text == Some("--"));
            if count != if dash_dash { 2 } else { 1 } {
                return false;
            }
            let argument = if inside.word_active {
                inside.word
            } else {
                arguments.last().and_then(|word| word.text)
            };
            return dash_dash || !argument.is_some_and(|word| word.starts_with('-'));
        }
    }
    false
}
