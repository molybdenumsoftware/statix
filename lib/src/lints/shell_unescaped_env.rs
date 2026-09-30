use super::optionals_string::unparen;
use super::shell_context::{Quote, escaped_arg, prefix, script_parts};
use crate::{
    Metadata, Report, Rule, Severity,
    utils::{has_local_value_binding, is_shadowed},
};
use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{Apply, Attr, Expr, InterpolPart, Str},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Advises about builtin `toString` interpolated as an unquoted value of a
/// straightforward `export NAME=...` assignment in a recognized shell script.
///
/// ## Why is this bad?
/// Conversion to a Nix string does not escape shell data. Spaces and shell
/// metacharacters can change the interpretation of an exported value.
/// [Home Manager's restic change](https://github.com/nix-community/home-manager/pull/9046)
/// centrally escapes environment values instead of using `toString`.
///
/// This is a conditional Hint, not a proof that a value contains unsafe data.
/// Review the intended shell semantics and consider `lib.escapeShellArg`
/// manually. No string is split or rewritten, and no library input is inserted.
/// Recognition is limited to the shell consumers and lexical forms supported
/// by `shell_context`: heredocs, backticks, arithmetic, unknown interpolations,
/// and nested `sh`/`bash -c` scripts are not analyzed. Quoted or opaque
/// assignment fragments, export options, non-assignment export arguments, and
/// shadowed builtins are excluded; this is not full shell analysis.
///
/// ## Example
/// ```nix
/// { lib, repository }: { script = ''export RESTIC_REPOSITORY=${toString repository}''; }
/// ```
#[lint(
    name = "shell_unescaped_env",
    note = "String conversion used for unquoted exported shell data",
    code = 35,
    match_with = SyntaxKind::NODE_STRING
)]
struct ShellUnescapedEnv;

impl Rule for ShellUnescapedEnv {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let string = Str::cast(node.clone())?;
        // Avoid normalization allocations for strings without a candidate call.
        if !string.parts().any(|part| match part {
            InterpolPart::Interpolation(hole) => hole.expr().and_then(to_string_call).is_some(),
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
            let Some(call) = hole.expr().and_then(to_string_call) else {
                continue;
            };
            if call.argument().is_some_and(escaped_arg) {
                continue;
            }
            let Some(context) = prefix(&parts, index) else {
                continue;
            };
            if context.comment
                || context.position.is_command()
                || context.quote != Quote::Unquoted
                || context.substitution_depth != 0
            {
                continue;
            }
            let Some(word) = context.word else {
                continue;
            };
            if !assignment(word) || !plain_assignment_suffix(&parts, index) {
                continue;
            }
            let Some(command) = context.words.iter().rposition(|word| word.command_start) else {
                continue;
            };
            let export = &context.words[command];
            if export.substitution_depth != 0 || export.quoted || export.text != Some("export") {
                continue;
            }
            if context.words[command + 1..].iter().any(|word| {
                word.substitution_depth != 0 || word.quoted || !word.text.is_some_and(assignment)
            }) {
                continue;
            }
            report = report.diagnostic(
                hole.syntax().text_range(),
                "If this exported value is shell data, `toString` does not shell-escape it; review the intended semantics and consider `lib.escapeShellArg` manually",
            );
        }
        (!report.diagnostics.is_empty()).then_some(report)
    }
}

fn plain_assignment_suffix(parts: &[InterpolPart<String>], index: usize) -> bool {
    let Some(part) = parts.get(index + 1) else {
        return true;
    };
    let InterpolPart::Literal(text) = part else {
        return false;
    };
    for byte in text.bytes() {
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' | b';' | b'|' | b'&' => return true,
            b'\'' | b'"' | b'\\' | b'$' | b'`' => return false,
            _ => {}
        }
    }
    index + 2 == parts.len()
}

fn assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn to_string_call(expr: Expr) -> Option<Apply> {
    let Expr::Apply(call) = unparen(expr)? else {
        return None;
    };
    call.argument()?;
    match unparen(call.lambda()?)? {
        Expr::Ident(ident) => {
            if ident.ident_token()?.text() != "toString"
                || is_shadowed(ident.syntax(), "toString")
                || has_local_value_binding(ident.syntax(), "toString")
            {
                return None;
            }
        }
        Expr::Select(select) => {
            if select.default_expr().is_some() || select.or_token().is_some() {
                return None;
            }
            let Expr::Ident(base) = unparen(select.expr()?)? else {
                return None;
            };
            if base.ident_token()?.text() != "builtins"
                || is_shadowed(base.syntax(), "builtins")
                || has_local_value_binding(base.syntax(), "builtins")
            {
                return None;
            }
            let path = select.attrpath()?;
            let mut attrs = path.attrs();
            let Attr::Ident(member) = attrs.next()? else {
                return None;
            };
            if member.ident_token()?.text() != "toString" || attrs.next().is_some() {
                return None;
            }
        }
        _ => return None,
    }
    Some(call)
}
