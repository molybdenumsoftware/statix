//! Bounded shell lexical context for advisory diagnostics, not a shell parser.
//!
//! Only direct, conventional shell attributes and qualified `pkgs` shell builders
//! are inspected. Nix escapes/indentation are decoded by rnix. Literal commands,
//! simple `for ... in ...; do ...; done` words and `$()` are recognized. Redirection
//! (including heredocs), backticks, arithmetic, parameter expansions, grouping,
//! functions, case/conditional grammar, and nested shell invocations are excluded.
//! Unknown Nix holes invalidate subsequent context; an escape helper is atomic
//! only in an unquoted shell word. No evaluation or shell rewrite is performed.

use rnix::ast::{Apply, AstToken, Attr, AttrpathValue, Expr, InterpolPart, Str};
use rowan::ast::AstNode as _;

use super::optionals_string::{enclosing_parens, lib_member, unparen};
use crate::utils::has_local_value_binding;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Quote {
    Unquoted,
    Single,
    Double,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Position {
    #[default]
    Argument,
    Command,
    Statement,
}

impl Position {
    pub(super) fn is_command(self) -> bool {
        self != Self::Argument
    }

    pub(super) fn is_statement(self) -> bool {
        self == Self::Statement
    }
}

#[derive(Debug)]
pub(super) struct ShellWord<'a> {
    pub text: Option<&'a str>,
    pub command_start: bool,
    pub statement_start: bool,
    pub quoted: bool,
    pub substitution_depth: usize,
}

#[derive(Debug)]
pub(super) struct Prefix<'a> {
    pub quote: Quote,
    pub substitution_depth: usize,
    pub word: Option<&'a str>,
    pub word_active: bool,
    pub words: Vec<ShellWord<'a>>,
    pub position: Position,
    pub comment: bool,
}

pub(super) fn escaped_arg(expr: Expr) -> bool {
    escaped_shape(expr, false)
}

fn escaped_shape(expr: Expr, singleton: bool) -> bool {
    let Some(Expr::Apply(apply)) = unparen(expr) else {
        return false;
    };
    let Some(argument) = apply.argument().and_then(unparen) else {
        return false;
    };
    let Some(callee) = apply.lambda() else {
        return false;
    };
    if lib_member(callee.clone(), "escapeShellArg").is_some() {
        return true;
    }
    let Expr::List(list) = argument else {
        return false;
    };
    let mut items = list.items();
    items.next().is_some()
        && (!singleton || items.next().is_none())
        && lib_member(callee, "escapeShellArgs").is_some()
}

fn shell_builder(expr: Expr, member: &str) -> bool {
    let Some(Expr::Select(select)) = unparen(expr) else {
        return false;
    };
    if select.default_expr().is_some() || select.or_token().is_some() {
        return false;
    }
    let Some(Expr::Ident(base)) = select.expr().and_then(unparen) else {
        return false;
    };
    if base
        .ident_token()
        .is_none_or(|token| token.text() != "pkgs")
        || has_local_value_binding(select.syntax(), "pkgs")
    {
        return false;
    }
    let Some(path) = select.attrpath() else {
        return false;
    };
    let mut attrs = path.attrs();
    matches!(attrs.next(), Some(Attr::Ident(attr)) if attr.ident_token().is_some_and(|token| token.text() == member))
        && attrs.next().is_none()
}

fn shell_consumer(string: &Str) -> Option<()> {
    let value = enclosing_parens(string.syntax().clone());
    if let Some(entry) = value.parent().and_then(AttrpathValue::cast) {
        if entry.value()?.syntax() != &value {
            return None;
        }
        let path = entry.attrpath()?;
        // A quoted/dynamic or multi-component attribute is not guessed to be shell.
        let mut attrs = path.attrs();
        let Attr::Ident(attr) = attrs.next()? else {
            return None;
        };
        if attrs.next().is_some() {
            return None;
        }
        let token = attr.ident_token()?;
        if matches!(
            token.text(),
            "script"
                | "preBuild"
                | "postBuild"
                | "buildPhase"
                | "installPhase"
                | "preInstall"
                | "postInstall"
                | "preConfigure"
                | "postConfigure"
                | "configurePhase"
                | "checkPhase"
                | "preCheck"
                | "postCheck"
                | "unpackPhase"
                | "patchPhase"
                | "fixupPhase"
                | "preFixup"
                | "postFixup"
                | "shellHook"
        ) {
            return Some(());
        }
        if token.text() != "text" {
            return None;
        }
        let attrs = enclosing_parens(entry.syntax().parent()?);
        let call = attrs.parent().and_then(Apply::cast)?;
        if call.argument()?.syntax() == &attrs
            && shell_builder(call.lambda()?, "writeShellApplication")
        {
            return Some(());
        }
        return None;
    }
    let call = value.parent().and_then(Apply::cast)?;
    if call.argument()?.syntax() != &value {
        return None;
    }
    let Expr::Apply(first) = unparen(call.lambda()?)? else {
        return None;
    };
    if !matches!(unparen(first.argument()?)?, Expr::Str(_)) {
        return None;
    }
    let callee = first.lambda()?;
    (shell_builder(callee.clone(), "writeShellScript")
        || shell_builder(callee, "writeShellScriptBin"))
    .then_some(())
}

pub(super) fn script_parts(string: &Str) -> Option<Vec<InterpolPart<String>>> {
    shell_consumer(string)?;
    // All consumers need either a Nix hole or literal command substitution.
    // Avoid allocating decoded strings for ordinary shell text with no candidate.
    if !string.parts().any(|part| match part {
        InterpolPart::Interpolation(_) => true,
        InterpolPart::Literal(text) => text.syntax().text().contains("$("),
    }) {
        return None;
    }
    let parts = string.normalized_parts();
    // Preflight the complete literal structure so a hint never precedes an
    // unsupported later heredoc/backtick/shell-string consumer. Holes are opaque
    // here; prefix() separately enforces their stronger trust requirements.
    let mut lexer = Lexer::new(false);
    for part in &parts {
        match part {
            InterpolPart::Literal(text) => lexer.literal(text)?,
            InterpolPart::Interpolation(_) => lexer.hole(),
        }
    }
    lexer.finish()?;
    Some(parts)
}

pub(super) fn prefix(parts: &[InterpolPart<String>], index: usize) -> Option<Prefix<'_>> {
    if !matches!(parts.get(index)?, InterpolPart::Interpolation(_)) {
        return None;
    }
    scan_prefix(parts, index, None, true)
}

pub(super) fn prefix_at(
    parts: &[InterpolPart<String>],
    index: usize,
    offset: usize,
    capture_words: bool,
) -> Option<Prefix<'_>> {
    let InterpolPart::Literal(text) = parts.get(index)? else {
        return None;
    };
    text.get(..offset)?;
    scan_prefix(parts, index, Some(offset), capture_words)
}

fn scan_prefix(
    parts: &[InterpolPart<String>],
    index: usize,
    offset: Option<usize>,
    capture_words: bool,
) -> Option<Prefix<'_>> {
    let mut lexer = Lexer::new(capture_words);
    for part in parts.get(..index)? {
        match part {
            InterpolPart::Literal(text) => lexer.literal(text)?,
            InterpolPart::Interpolation(hole) => {
                if lexer.quote != Quote::Unquoted
                    || lexer.comment
                    || lexer.escaped
                    || !escaped_shape(hole.expr()?, true)
                {
                    return None;
                }
                lexer.hole();
            }
        }
    }
    if let Some(offset) = offset {
        let InterpolPart::Literal(text) = &parts[index] else {
            return None;
        };
        lexer.literal(text.get(..offset)?)?;
    }
    if lexer.escaped {
        return None;
    }
    Some(lexer.context())
}

#[derive(Default)]
struct Word<'a> {
    literal: Option<&'a str>,
    start: usize,
    end: usize,
    active: bool,
    opaque: bool,
    quoted: bool,
    position: Position,
}

struct Frame<'a> {
    quote: Quote,
    word: Word<'a>,
    position: Position,
}

struct Lexer<'a> {
    quote: Quote,
    escaped: bool,
    comment: bool,
    position: Position,
    word: Word<'a>,
    words: Vec<ShellWord<'a>>,
    capture_words: bool,
    stack: Vec<Frame<'a>>,
}

impl<'a> Lexer<'a> {
    fn new(capture_words: bool) -> Self {
        Self {
            quote: Quote::Unquoted,
            escaped: false,
            comment: false,
            position: Position::Statement,
            word: Word::default(),
            words: Vec::new(),
            capture_words,
            stack: Vec::new(),
        }
    }

    fn begin(&mut self) {
        if !self.word.active {
            self.word.active = true;
            self.word.position = self.position;
        }
    }

    fn byte(&mut self, text: &'a str, start: usize, end: usize) {
        self.begin();
        match self.word.literal {
            None if !self.word.opaque => {
                self.word.literal = Some(text);
                self.word.start = start;
            }
            Some(previous) if !std::ptr::eq(previous.as_ptr(), text.as_ptr()) => {
                self.word.opaque = true;
            }
            _ => {}
        }
        self.word.end = end;
    }

    fn hole(&mut self) {
        if !self.comment {
            self.begin();
            self.word.opaque = true;
        }
    }

    fn complete_word(&mut self) -> Option<()> {
        if !self.word.active {
            return Some(());
        }
        let text = self.word_text();
        if self.word.position.is_command()
            && matches!(
                text,
                Some(
                    "sh" | "bash"
                        | "dash"
                        | "zsh"
                        | "ksh"
                        | "eval"
                        | "source"
                        | "."
                        | "if"
                        | "then"
                        | "else"
                        | "elif"
                        | "fi"
                        | "case"
                        | "esac"
                        | "while"
                        | "until"
                        | "select"
                        | "function"
                        | "["
                        | "[["
                )
            )
        {
            return None;
        }
        // Quoted/escaped command names are opaque, not interpreted as a safe
        // consumer. Likewise a hole in command position has unknown semantics.
        if self.word.position.is_command() && text.is_none() {
            return None;
        }
        if self.capture_words {
            self.words.push(ShellWord {
                text,
                command_start: self.word.position.is_command(),
                statement_start: self.word.position.is_statement(),
                quoted: self.word.quoted,
                substitution_depth: self.stack.len(),
            });
        }
        let assignment = text.is_some_and(|word| {
            word.split_once('=')
                .is_some_and(|(name, _)| identifier(name))
        });
        self.position = if assignment {
            self.word.position
        } else if self.word.position.is_command() && text == Some("do") {
            Position::Command
        } else {
            Position::Argument
        };
        self.word = Word::default();
        Some(())
    }

    fn word_text(&self) -> Option<&'a str> {
        if self.word.opaque || !self.word.active {
            return None;
        }
        self.word.literal?.get(self.word.start..self.word.end)
    }

    fn dollar_token(&mut self, next: &[u8]) -> Option<bool> {
        match next.first() {
            Some(b'(') => {
                if next.get(1) == Some(&b'(') || self.stack.len() >= 32 {
                    return None;
                }
                self.begin();
                self.word.opaque = true;
                let word = std::mem::take(&mut self.word);
                self.stack.push(Frame {
                    quote: self.quote,
                    word,
                    position: self.position,
                });
                self.quote = Quote::Unquoted;
                self.position = Position::Statement;
                return Some(true);
            }
            Some(b'{') => return None,
            Some(next) if next.is_ascii_alphanumeric() || *next == b'_' => {
                self.begin();
                self.word.opaque = true;
            }
            Some(b'?' | b'#' | b'@' | b'*' | b'!' | b'$' | b'-') => {
                self.begin();
                self.word.opaque = true;
            }
            _ => {}
        }
        Some(false)
    }

    fn protected_byte(&mut self, byte: u8, text: &'a str, index: usize) -> bool {
        if self.comment {
            if byte == b'\n' {
                self.comment = false;
                self.position = Position::Statement;
            }
        } else if self.escaped {
            self.escaped = false;
            self.word.opaque = true;
        } else if self.quote == Quote::Single {
            if byte == b'\'' {
                self.quote = Quote::Unquoted;
            }
            self.byte(text, index, index + 1);
        } else {
            return false;
        }
        true
    }

    fn literal(&mut self, text: &'a str) -> Option<()> {
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let byte = bytes[i];
            if self.protected_byte(byte, text, i) {
                i += 1;
                continue;
            }
            if byte == b'\\' {
                self.begin();
                self.word.opaque = true;
                // In double quotes a backslash only escapes these characters.
                if self.quote == Quote::Unquoted
                    || bytes
                        .get(i + 1)
                        .is_none_or(|next| matches!(next, b'$' | b'`' | b'"' | b'\\' | b'\n'))
                {
                    self.escaped = true;
                }
                i += 1;
                continue;
            }
            if byte == b'`' {
                return None;
            }
            if byte == b'"' {
                self.begin();
                self.word.opaque = true;
                self.word.quoted = true;
                self.quote = if self.quote == Quote::Double {
                    Quote::Unquoted
                } else {
                    Quote::Double
                };
                i += 1;
                continue;
            }
            if self.quote == Quote::Unquoted && byte == b'\'' {
                self.begin();
                self.word.opaque = true;
                self.word.quoted = true;
                self.quote = Quote::Single;
                i += 1;
                continue;
            }
            if byte == b'$' && self.dollar_token(&bytes[i + 1..])? {
                i += 2;
                continue;
            }
            if self.quote == Quote::Double {
                self.byte(text, i, i + 1);
                i += 1;
                continue;
            }
            match byte {
                b' ' | b'\t' | b'\r' => self.complete_word()?,
                b'\n' | b';' => {
                    self.complete_word()?;
                    if byte == b';' && bytes.get(i + 1) == Some(&b';') {
                        return None;
                    }
                    self.position = Position::Statement;
                }
                b'|' | b'&' => {
                    self.complete_word()?;
                    if byte == b'&' && bytes.get(i + 1) != Some(&b'&') {
                        return None;
                    }
                    if bytes.get(i + 1) == Some(&byte) {
                        i += 1;
                    }
                    self.position = Position::Command;
                }
                b'#' if !self.word.active => self.comment = true,
                b')' => {
                    self.complete_word()?;
                    let frame = self.stack.pop()?;
                    self.quote = frame.quote;
                    self.word = frame.word;
                    self.position = frame.position;
                }
                b'(' | b'<' | b'>' | b'{' | b'}' => return None,
                _ => self.byte(text, i, i + 1),
            }
            i += 1;
        }
        Some(())
    }

    fn finish(mut self) -> Option<()> {
        if self.quote != Quote::Unquoted || self.escaped || !self.stack.is_empty() {
            return None;
        }
        self.complete_word()
    }

    fn context(self) -> Prefix<'a> {
        Prefix {
            quote: self.quote,
            substitution_depth: self.stack.len(),
            word: self.word_text(),
            word_active: self.word.active,
            position: if self.word.active {
                self.word.position
            } else {
                self.position
            },
            comment: self.comment,
            words: self.words,
        }
    }
}

fn identifier(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn string(source: &str) -> Str {
        let parsed = rnix::Root::parse(source);
        assert!(
            parsed.errors().is_empty(),
            "invalid Nix fixture: {:?}",
            parsed.errors()
        );
        parsed
            .syntax()
            .descendants()
            .find_map(Str::cast)
            .expect("fixture string")
    }

    fn hole(parts: &[InterpolPart<String>], ordinal: usize) -> usize {
        parts
            .iter()
            .enumerate()
            .filter(|(_, part)| matches!(part, InterpolPart::Interpolation(_)))
            .nth(ordinal)
            .expect("fixture hole")
            .0
    }

    #[test]
    fn quote_state_and_opaque_words_follow_shell_not_nix_quotes() {
        for (source, quote, active) in [
            (r#"{ script = "echo ${x}"; }"#, Quote::Unquoted, false),
            (r#"{ script = "echo '${x}'"; }"#, Quote::Single, true),
            (r#"{ script = "echo \"${x}\""; }"#, Quote::Double, true),
            (
                r#"{ script = "echo it\\'s ${x}"; }"#,
                Quote::Unquoted,
                false,
            ),
            (
                r#"{ script = "echo \\\"${x}\\\""; }"#,
                Quote::Unquoted,
                true,
            ),
        ] {
            let parts = string(source).normalized_parts();
            let context = prefix(&parts, hole(&parts, 0)).expect(source);
            assert_eq!(context.quote, quote, "{source}");
            assert_eq!(context.word_active, active, "{source}");
            assert!(!context.position.is_command(), "{source}");
            assert_eq!(context.words[0].text, Some("echo"), "{source}");
            assert!(context.words[0].command_start, "{source}");
            assert_eq!(context.word, None, "{source}");
        }
    }

    #[test]
    fn assignment_prefix_is_plain_only_until_composition() {
        for (source, word) in [
            (r#"{ script = "export NAME=${x}"; }"#, Some("NAME=")),
            (r#"{ script = "export NAME='pre'${x}"; }"#, None),
            (r#"{ script = "export NAME=pre\\ space${x}"; }"#, None),
            (r#"{ script = "export NAME=$(printf pre)${x}"; }"#, None),
        ] {
            let parts = string(source).normalized_parts();
            let context = prefix(&parts, hole(&parts, 0)).expect(source);
            assert_eq!(context.word, word, "{source}");
            assert_eq!(context.quote, Quote::Unquoted, "{source}");
            assert!(!context.position.is_command(), "{source}");
        }
    }

    #[test]
    fn substitution_restores_parent_quote_and_records_child_depth() {
        let parts =
            string(r#"{ lib }: { script = "echo \"$(dirname ${lib.escapeShellArg x})\" ${y}"; }"#)
                .normalized_parts();
        let inside = prefix(&parts, hole(&parts, 0)).unwrap();
        assert_eq!(inside.quote, Quote::Unquoted);
        assert_eq!(inside.substitution_depth, 1);
        assert_eq!(inside.words[0].substitution_depth, 0);
        assert_eq!(inside.words[1].text, Some("dirname"));
        assert!(inside.words[1].command_start);
        assert_eq!(inside.words[1].substitution_depth, 1);
        let after = prefix(&parts, hole(&parts, 1)).unwrap();
        assert_eq!(after.quote, Quote::Unquoted);
        assert_eq!(after.substitution_depth, 0);
        assert!(
            after
                .words
                .iter()
                .any(|word| word.quoted && word.substitution_depth == 0)
        );

        let parts =
            string(r#"{ lib }: { script = "echo \"$(dirname ${lib.escapeShellArg x})${y}\""; }"#)
                .normalized_parts();
        let after = prefix(&parts, hole(&parts, 1)).unwrap();
        assert_eq!(after.quote, Quote::Double);
        assert_eq!(after.substitution_depth, 0);
        assert!(after.word_active);
        assert_eq!(after.word, None);

        let parts = string(r#"{ lib }: { script = "echo $(dirname $(printf '%s' ${lib.escapeShellArg x})) ${y}"; }"#).normalized_parts();
        let inside = prefix(&parts, hole(&parts, 0)).unwrap();
        assert_eq!(inside.substitution_depth, 2);
        assert!(
            inside
                .words
                .iter()
                .any(|word| word.text == Some("dirname") && word.substitution_depth == 1)
        );
        assert!(
            inside
                .words
                .iter()
                .any(|word| word.text == Some("printf") && word.substitution_depth == 2)
        );
        let after = prefix(&parts, hole(&parts, 1)).unwrap();
        assert_eq!(after.substitution_depth, 0);
        assert_eq!(after.quote, Quote::Unquoted);
    }

    #[test]
    fn root_loop_words_and_real_statement_boundaries_are_preserved() {
        let parts = string(r#"{ script = "for x in one two; do export NAME=${x}; done"; }"#)
            .normalized_parts();
        let context = prefix(&parts, hole(&parts, 0)).unwrap();
        let words = &context.words;
        assert_eq!(
            words.iter().map(|word| word.text).collect::<Vec<_>>(),
            vec![
                Some("for"),
                Some("x"),
                Some("in"),
                Some("one"),
                Some("two"),
                Some("do"),
                Some("export")
            ]
        );
        assert!(words[0].command_start && words[0].statement_start);
        assert!(words[5].command_start && words[5].statement_start);
        assert!(words[6].command_start && !words[6].statement_start);
        assert!(words.iter().all(|word| word.substitution_depth == 0));
        for (separator, expected_command, expected_statement) in [
            (" ", false, false),
            ("; ", true, true),
            (" && ", true, false),
            (" | ", true, false),
        ] {
            let source = format!("{{ script = \"for x in one{separator}do echo ${{x}}\"; }}");
            let parts = string(&source).normalized_parts();
            let context = prefix(&parts, hole(&parts, 0)).unwrap();
            let do_word = context
                .words
                .iter()
                .find(|word| word.text == Some("do"))
                .unwrap();
            assert_eq!(do_word.command_start, expected_command, "{source}");
            assert_eq!(do_word.statement_start, expected_statement, "{source}");
        }
    }

    #[test]
    fn comments_do_not_supply_words_or_quote_delimiters() {
        let parts =
            string(r#"{ script = "echo ok # '\"$(ignored\ncat ${x}"; }"#).normalized_parts();
        let context = prefix(&parts, hole(&parts, 0)).unwrap();
        assert_eq!(context.quote, Quote::Unquoted);
        assert!(!context.comment);
        assert_eq!(
            context
                .words
                .iter()
                .map(|word| word.text)
                .collect::<Vec<_>>(),
            vec![Some("echo"), Some("ok"), Some("cat")]
        );
        assert!(context.words[2].statement_start);
        let parts = string(r##"{ script = "# docs '${x}'"; }"##).normalized_parts();
        assert!(prefix(&parts, hole(&parts, 0)).unwrap().comment);
        let parts = string(r#"{ script = "echo '#literal' ${x}"; }"#).normalized_parts();
        let context = prefix(&parts, hole(&parts, 0)).unwrap();
        assert!(!context.comment);
        assert!(context.words[1].quoted);
        assert_eq!(context.words[1].text, None);
    }

    #[test]
    fn only_atomic_unquoted_escape_helpers_preserve_later_context() {
        for source in [
            r#"{ lib }: { script = "echo ${lib.escapeShellArg x}; cat ${y}"; }"#,
            r#"{ lib }: { script = "echo ${lib.escapeShellArgs [ x ]}; cat ${y}"; }"#,
        ] {
            let parts = string(source).normalized_parts();
            let context = prefix(&parts, hole(&parts, 1)).expect(source);
            assert_eq!(context.quote, Quote::Unquoted);
            assert_eq!(context.words.last().unwrap().text, Some("cat"));
            assert_eq!(context.words[1].text, None);
        }
        for source in [
            r#"{ script = "echo ${x}; cat ${y}"; }"#,
            r#"{ lib }: { script = "echo '${lib.escapeShellArg x}'; cat ${y}"; }"#,
            r#"{ lib }: { script = "echo \"${lib.escapeShellArg x}\"; cat ${y}"; }"#,
            r#"{ lib }: { script = "echo ${lib.escapeShellArgs [ x y ]}; cat ${y}"; }"#,
            r#"{ lib }: { script = "echo ${lib.escapeShellArgs []}; cat ${y}"; }"#,
            r#"{ lib }: { script = "echo ${lib.escapeShellArgs xs}; cat ${y}"; }"#,
            r##"{ script = "# ${x}\ncat ${y}"; }"##,
            r#"{ lib }: let lib = custom; in { script = "echo ${lib.escapeShellArg x}; cat ${y}"; }"#,
        ] {
            let parts = string(source).normalized_parts();
            assert!(prefix(&parts, hole(&parts, 1)).is_none(), "{source}");
        }
    }

    #[test]
    fn normalized_utf8_offsets_and_wrong_part_kinds_are_checked() {
        let parts = vec![InterpolPart::Literal("echo é".to_owned())];
        assert!(prefix_at(&parts, 0, 6, false).is_none());
        assert_eq!(prefix_at(&parts, 0, 7, false).unwrap().word, Some("é"));
        assert!(prefix_at(&parts, 0, 8, false).is_none());
        assert!(prefix(&parts, 0).is_none());
        assert!(prefix_at(&parts, 1, 0, false).is_none());
        let parts = string(r#"{ script = "echo ${x}"; }"#).normalized_parts();
        assert!(prefix_at(&parts, hole(&parts, 0), 0, false).is_none());
    }
}
