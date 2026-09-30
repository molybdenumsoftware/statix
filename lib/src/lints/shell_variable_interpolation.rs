use super::shell_context::{Prefix, Quote, prefix, script_parts};
use crate::{
    Metadata, Report, Rule, Severity,
    utils::{has_local_value_binding, is_shadowed},
};
use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{Expr, Ident, InterpolPart, Str},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Advises when a bare, lexically unbound Nix identifier is interpolated after
/// a matching, straightforward root-shell `for NAME in ...; do` binding.
///
/// ## Why is this bad?
/// Nix evaluates `${x}` before the shell runs. If the shell variable was
/// intended, the interpolation must instead be escaped for the surrounding Nix
/// string. The [NixOS Discourse answer](https://discourse.nixos.org/t/dealing-with-shell-variables-in-nix-strings/7978/2)
/// describes `''${x}` in indented strings and notes that `$x` needs no escape.
///
/// This conditional Hint does not assume the user's intended variable, split
/// strings, or insert escapes. Single-quoted shell sites are skipped because
/// escaping Nix interpolation there would still not expand the shell variable.
/// Recognition uses completed unquoted shell words, not substring matching.
/// Only explicit `for NAME in ITEMS; do` or newline-separated headers are
/// recognized; implicit argument loops, arithmetic loops, other control
/// structures, aliases, unknown prior Nix holes, and child-shell bindings are
/// not inferred. Heredocs, backticks, arithmetic, and nested `sh`/`bash -c`
/// scripts are outside the shared lexer. This is not full shell analysis.
///
/// ## Example
/// ```nix
/// { script = ''for x in one two; do printf '%s\n' "${x}"; done''; }
/// ```
#[lint(
    name = "shell_variable_interpolation",
    note = "Nix interpolation may refer to an intended shell loop variable",
    code = 36,
    match_with = SyntaxKind::NODE_STRING
)]
struct ShellVariableInterpolation;

impl Rule for ShellVariableInterpolation {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let string = Str::cast(node.clone())?;
        // Check candidate AST shapes before allocating normalized shell parts.
        if !string.parts().any(|part| match part {
            InterpolPart::Interpolation(hole) => hole.expr().and_then(unbound_ident).is_some(),
            InterpolPart::Literal(_) => false,
        }) {
            return None;
        }
        let parts = script_parts(&string)?;
        let mut report = self.report().severity(Severity::Hint);
        for (index, part) in parts.iter().enumerate() {
            let InterpolPart::Interpolation(hole) = part else {
                continue;
            };
            let Some(ident) = hole.expr().and_then(unbound_ident) else {
                continue;
            };
            let Some(token) = ident.ident_token() else {
                continue;
            };
            let Some(context) = prefix(&parts, index) else {
                continue;
            };
            if context.comment
                || context.quote == Quote::Single
                || context.substitution_depth != 0
                || !preceding_loop_binding(&context, token.text())
            {
                continue;
            }
            report = report.diagnostic(
                hole.syntax().text_range(),
                "If this names the preceding shell loop variable, escape the Nix interpolation for this string (or use `$name`); Nix evaluates `${name}` before the shell runs",
            );
        }
        (!report.diagnostics.is_empty()).then_some(report)
    }
}

fn unbound_ident(expr: Expr) -> Option<Ident> {
    let Expr::Ident(ident) = expr else {
        return None;
    };
    let token = ident.ident_token()?;
    let name = token.text();
    if matches!(name, "builtins" | "true" | "false" | "null")
        || is_shadowed(ident.syntax(), name)
        || has_local_value_binding(ident.syntax(), name)
    {
        return None;
    }
    Some(ident)
}

fn shell_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

enum Header<'a> {
    None,
    Name,
    In(&'a str),
    Items { name: &'a str, nonempty: bool },
}

fn preceding_loop_binding(context: &Prefix<'_>, target: &str) -> bool {
    let mut header = Header::None;
    let mut bound = false;
    for (index, word) in context.words.iter().enumerate() {
        if word.substitution_depth != 0 {
            // Child commands cannot establish root bindings. A child command
            // embedded in a loop header is outside this bounded recognition.
            if !matches!(header, Header::None) {
                return false;
            }
            continue;
        }
        if word.command_start {
            if !word.quoted && word.text == Some("for") {
                // Pipelines and boolean-command operands are outside the
                // supported loop-binding scope. A directly nested loop after
                // an already confirmed `do` is still a root-shell statement.
                if !word.statement_start
                    && !index.checked_sub(1).is_some_and(|previous| {
                        let previous = &context.words[previous];
                        previous.substitution_depth == 0
                            && previous.command_start
                            && !previous.quoted
                            && previous.text == Some("do")
                    })
                {
                    return false;
                }
                if !matches!(header, Header::None) {
                    return false;
                }
                header = Header::Name;
                continue;
            }
            if !word.quoted && word.text == Some("do") {
                if !word.statement_start {
                    return false;
                }
                let Header::Items {
                    name,
                    nonempty: true,
                } = header
                else {
                    return false;
                };
                bound |= name == target;
                header = Header::None;
                continue;
            }
            if !matches!(header, Header::None) {
                return false;
            }
            if !word.quoted
                && word.text.is_some_and(|text| {
                    matches!(
                        text,
                        "while"
                            | "until"
                            | "if"
                            | "then"
                            | "else"
                            | "elif"
                            | "fi"
                            | "case"
                            | "esac"
                            | "select"
                            | "function"
                            | "{"
                            | "}"
                    )
                })
            {
                return false;
            }
            continue;
        }
        header = match header {
            Header::None => Header::None,
            Header::Name => {
                let Some(name) = word.text.filter(|name| !word.quoted && shell_name(name)) else {
                    return false;
                };
                Header::In(name)
            }
            Header::In(name) => {
                if word.quoted || word.text != Some("in") {
                    return false;
                }
                Header::Items {
                    name,
                    nonempty: false,
                }
            }
            Header::Items { name, .. } => Header::Items {
                name,
                nonempty: true,
            },
        };
    }
    // Shell loop variables remain in the root environment after `done`.
    bound && matches!(header, Header::None)
}
