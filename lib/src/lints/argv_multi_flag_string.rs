use super::optionals_string::{enclosing_parens, unparen};
use crate::{Metadata, Report, Rule, Severity};
use macros::lint;
use rnix::{
    SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{AstToken, Attr, AttrSet, AttrpathValue, Expr, InterpolPart, Lambda, LetIn, List, Str},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Advises when a plain literal launchd argument starts with a flag and contains
/// another whitespace-separated flag token. Recognizes only Home Manager's
/// `launchd.agents.<literal-name>.config.ProgramArguments`, optionally under
/// an explicit module `config`, including equivalent nested attribute sets.
///
/// ## Why is this bad?
/// Each `ProgramArguments` element is one argument, not shell command text.
/// Home Manager's nh agent passed `"--keep 5 --keep-since 3d"` as one argument
/// and failed; see <https://github.com/nix-community/home-manager/pull/9907>.
/// This is a hint, not a proof of a command's argument grammar. No splitting
/// fix is offered because a space-containing value can be intentional.
///
/// Only directly assigned literal lists with plain ordinary string elements
/// are inspected. Quoting, escapes, interpolations, indented strings, computed
/// elements, unknown consumer paths, and known command interpreters/wrappers
/// are excluded. Arbitrary applications and inner data sets are not treated as
/// root launchd configuration merely because their keys have familiar names.
///
/// ## Example
/// ```nix
/// { launchd.agents.example.config.ProgramArguments =
///     [ "nh" "clean" "user" "--keep 5 --keep-since 3d" ]; }
/// ```
#[lint(
    name = "argv_multi_flag_string",
    note = "Multiple flags in one launchd argument",
    code = 32,
    match_with = SyntaxKind::NODE_STRING
)]
struct ArgvMultiFlagString;

impl Rule for ArgvMultiFlagString {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let string = Str::cast(node.as_node()?.clone())?;
        let literal = plain_literal(&string)?;
        if !multiple_flags(literal.text()) {
            return None;
        }
        let argument = enclosing_parens(string.syntax().clone());
        let list = List::cast(argument.parent()?)?;
        if !launchd_arguments(list.syntax()) {
            return None;
        }
        let mut items = list.items();
        let executable = items.next()?;
        if executable.syntax() == &argument {
            return None;
        }
        let Expr::Str(executable) = unparen(executable)? else {
            return None;
        };
        let executable = plain_literal(&executable)?;
        let basename = executable.text().rsplit('/').next()?;
        // Interpreter programs and launch wrappers may consume later strings
        // as source code rather than flags. Do not guess their operand grammar.
        if command_consumer(basename) {
            return None;
        }
        let mut positional = false;
        for item in items {
            let candidate = item.syntax() == &argument;
            let Expr::Str(string) = unparen(item)? else {
                return None;
            };
            let literal = plain_literal(&string)?;
            if candidate && positional {
                return None;
            }
            positional |= literal.text() == "--";
        }
        Some(self.report().severity(Severity::Hint).diagnostic(
            string.syntax().text_range(),
            "Each ProgramArguments element is one argument; check whether these flags need separate elements",
        ))
    }
}

/// Borrow the unnormalized token only when no interpretation is necessary.
fn plain_literal(string: &Str) -> Option<SyntaxToken> {
    if string.syntax().first_token()?.text() != "\"" || string.syntax().last_token()?.text() != "\""
    {
        return None;
    }
    let mut parts = string.parts();
    let InterpolPart::Literal(literal) = parts.next()? else {
        return None;
    };
    if parts.next().is_some()
        || literal.syntax().text().chars().any(|ch| {
            matches!(ch, '\\' | '\'' | '"' | '`' | '\n' | '\r')
                || (ch.is_whitespace() && !ch.is_ascii_whitespace())
        })
    {
        return None;
    }
    Some(literal.syntax().clone())
}

fn multiple_flags(text: &str) -> bool {
    if !text.starts_with('-') {
        return false;
    }
    let mut words = text.split_ascii_whitespace();
    words.next().is_some_and(flag_token) && words.any(flag_token)
}

fn flag_token(word: &str) -> bool {
    let Some(name) = word.strip_prefix("--").or_else(|| word.strip_prefix('-')) else {
        return false;
    };
    let name = name.split('=').next().unwrap_or_default();
    name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn command_consumer(name: &str) -> bool {
    matches!(
        name,
        "sh" | "bash"
            | "zsh"
            | "dash"
            | "ksh"
            | "fish"
            | "csh"
            | "tcsh"
            | "eval"
            | "env"
            | "python"
            | "python2"
            | "python3"
            | "perl"
            | "ruby"
            | "node"
            | "nodejs"
            | "php"
            | "lua"
            | "luajit"
            | "osascript"
            | "nix"
            | "nix-instantiate"
            | "awk"
            | "gawk"
            | "mawk"
            | "sed"
            | "jq"
    ) || name.strip_prefix("python3.").is_some_and(|version| {
        !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit())
    })
}

/// Consume the exact consumer path across direct attribute-set boundaries.
/// At most one leading `config` is permitted; other outer data keys reject it.
fn launchd_arguments(node: &SyntaxNode) -> bool {
    let mut current = enclosing_parens(node.clone());
    let mut remaining = 5usize;
    let mut module_config = false;
    loop {
        let Some(entry) = current.parent().and_then(AttrpathValue::cast) else {
            return false;
        };
        if !entry
            .value()
            .is_some_and(|value| value.syntax() == &current)
        {
            return false;
        }
        let Some(path) = entry.attrpath() else {
            return false;
        };
        let count = path.attrs().count();
        if count == 0 || count > remaining + usize::from(!module_config) {
            return false;
        }
        let prefix = count > remaining;
        for (index, attr) in path.attrs().enumerate() {
            let expected = remaining + index;
            let matches = if expected < count {
                !module_config && static_key(&attr, "config")
            } else {
                match expected - count {
                    0 => static_key(&attr, "launchd"),
                    1 => static_key(&attr, "agents"),
                    2 => static_agent_name(&attr),
                    3 => static_key(&attr, "config"),
                    4 => static_key(&attr, "ProgramArguments"),
                    _ => false,
                }
            };
            if !matches {
                return false;
            }
        }
        remaining = remaining.saturating_sub(count);
        module_config |= prefix;
        let Some(set) = entry.syntax().parent().and_then(AttrSet::cast) else {
            return false;
        };
        current = enclosing_parens(set.syntax().clone());
        if current.parent().and_then(AttrpathValue::cast).is_none() {
            return remaining == 0 && root_configuration(current);
        }
    }
}

fn static_key(attr: &Attr, name: &str) -> bool {
    match attr {
        Attr::Ident(ident) => ident
            .ident_token()
            .is_some_and(|token| token.text() == name),
        Attr::Str(string) => plain_literal(string).is_some_and(|token| token.text() == name),
        Attr::Dynamic(_) => false,
    }
}

fn static_agent_name(attr: &Attr) -> bool {
    match attr {
        Attr::Ident(ident) => ident.ident_token().is_some(),
        Attr::Str(string) => plain_literal(string).is_some_and(|token| !token.text().is_empty()),
        Attr::Dynamic(_) => false,
    }
}

fn root_configuration(mut current: SyntaxNode) -> bool {
    loop {
        let Some(parent) = current.parent() else {
            return false;
        };
        if parent.kind() == SyntaxKind::NODE_ROOT {
            return true;
        }
        let body = if let Some(lambda) = Lambda::cast(parent.clone()) {
            lambda.body()
        } else if let Some(let_in) = LetIn::cast(parent.clone()) {
            let_in.body()
        } else {
            return false;
        };
        if !body.is_some_and(|body| body.syntax() == &current) {
            return false;
        }
        current = enclosing_parens(parent);
    }
}
