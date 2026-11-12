//! Render Nix strings as the shell scripts they evaluate to, remembering the
//! source byte range of every char so results can be mapped back.

use super::fixes::Change;
use rnix::ast::{self, AstToken as _, InterpolPart};
use rowan::ast::AstNode as _;

/// Rendered in place of `${...}`; a bare word stays valid in commands, paths
/// and arithmetic.
pub const PLACEHOLDER: &str = "__nix_interp__";

/// How to edit a shell script that lives in a Nix string.
pub const NIX_ESCAPING: &str = "`shellcheck` and `ruff` findings are in scripts written as Nix strings. \
Keep `${...}` interpolations as they are. In `''...''` strings write `''${` for a literal `${` \
and `'''` for `''`; in `\"...\"` strings escape `\"` and `\\` and write `\\${` for a literal `${`.";

/// Source bytes a rendered char came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Src {
    pub start: usize,
    pub end: usize,
    /// Part of a `${...}` placeholder.
    pub interp: bool,
}

#[derive(Debug)]
pub struct Line {
    pub chars: Vec<Src>,
    /// Source offset of the line end (its `\n`, or the closing quote).
    pub eol: usize,
}

#[derive(Debug)]
pub struct Script {
    pub text: String,
    pub lines: Vec<Line>,
    pub indented: bool,
    /// Indentation stripped from `''` strings.
    pub indent: usize,
}

enum Item {
    Char { c: char, src: Src, escaped: bool },
    Interp { src: Src },
}

impl Script {
    pub fn new(s: &ast::Str) -> Self {
        let indented = s.syntax().first_token().is_some_and(|t| t.text() == "''");
        let end: usize = s
            .syntax()
            .last_token()
            .map_or_else(|| s.syntax().text_range().end(), |t| t.text_range().start())
            .into();

        let mut items = Vec::new();
        for part in s.parts() {
            match part {
                InterpolPart::Literal(lit) => {
                    let tok = lit.syntax();
                    decode(
                        tok.text(),
                        tok.text_range().start().into(),
                        indented,
                        &mut items,
                    );
                }
                InterpolPart::Interpolation(i) => {
                    let r = i.syntax().text_range();
                    items.push(Item::Interp {
                        src: Src {
                            start: r.start().into(),
                            end: r.end().into(),
                            interp: true,
                        },
                    });
                }
            }
        }

        // Split into lines, remembering where each line ends in the source.
        let mut raw: Vec<(Vec<Item>, usize)> = vec![(Vec::new(), end)];
        for item in items {
            if let Item::Char { c: '\n', src, .. } = item {
                raw.last_mut().unwrap().1 = src.start;
                raw.push((Vec::new(), end));
            } else {
                raw.last_mut().unwrap().0.push(item);
            }
        }

        let mut indent = 0;
        if indented {
            let is_indent = |i: &Item| {
                matches!(
                    i,
                    Item::Char {
                        c: ' ',
                        escaped: false,
                        ..
                    }
                )
            };
            if raw.len() > 1 && raw[0].0.iter().all(is_indent) {
                raw.remove(0);
            }
            indent = raw
                .iter()
                .filter(|(l, _)| !l.iter().all(is_indent))
                .map(|(l, _)| l.iter().take_while(|i| is_indent(i)).count())
                .min()
                .unwrap_or(0);
            for (l, _) in &mut raw {
                let n = l.iter().take_while(|i| is_indent(i)).count().min(indent);
                l.drain(..n);
                if l.iter().all(is_indent) {
                    l.clear();
                }
            }
        }

        let mut text = String::new();
        let mut lines = Vec::with_capacity(raw.len());
        for (i, (items, eol)) in raw.into_iter().enumerate() {
            if i > 0 {
                text.push('\n');
            }
            let mut chars = Vec::new();
            for item in items {
                match item {
                    Item::Char { c, src, .. } => {
                        text.push(c);
                        chars.push(src);
                    }
                    Item::Interp { src } => {
                        text.push_str(PLACEHOLDER);
                        chars.extend(std::iter::repeat_n(src, PLACEHOLDER.len()));
                    }
                }
            }
            lines.push(Line { chars, eol });
        }
        Self {
            text,
            lines,
            indented,
            indent,
        }
    }

    /// Source offset of a 1-based shellcheck (line, column); `exact` is false
    /// when it points into a `${...}` placeholder or past the line end.
    pub fn source_offset(&self, line: usize, col: usize, is_end: bool) -> Option<(usize, bool)> {
        let l = self.lines.get(line.checked_sub(1)?)?;
        let idx = col.checked_sub(1)?;
        let Some(src) = l.chars.get(idx) else {
            return Some((l.eol, idx == l.chars.len()));
        };
        if !src.interp {
            return Some((src.start, true));
        }
        let starts_interp = idx == 0 || l.chars[idx - 1] != *src;
        if starts_interp {
            Some((src.start, true))
        } else if is_end {
            Some((src.end, false))
        } else {
            Some((src.start, false))
        }
    }

    /// Byte offset in `self.text` of a 1-based (line, column).
    pub fn text_offset(&self, line: usize, col: usize) -> Option<usize> {
        super::text_offset(&self.text, line, col)
    }

    /// Source range of the rendered text between byte offsets `start..end`.
    pub fn source_range(&self, start: usize, end: usize) -> Option<rnix::TextRange> {
        let position = |byte: usize| {
            let before = &self.text[..byte];
            let line = before.matches('\n').count() + 1;
            let col = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
            (line, col)
        };
        let (l1, c1) = position(start);
        let (l2, c2) = position(end);
        let (a, _) = self.source_offset(l1, c1, false)?;
        let (b, _) = self.source_offset(l2, c2, true)?;
        Some(rnix::TextRange::new(
            rnix::TextSize::try_from(a).ok()?,
            rnix::TextSize::try_from(b.max(a)).ok()?,
        ))
    }

    /// Whether a 1-based line of the script contains a `${...}` placeholder.
    pub fn line_has_interp(&self, line: usize) -> bool {
        self.lines
            .get(line.wrapping_sub(1))
            .is_some_and(|l| l.chars.iter().any(|c| c.interp))
    }

    /// `s` (the string this script was rendered from) with `changes` applied,
    /// if they can be expressed in the Nix source and it renders to exactly
    /// the intended script. Interpolations may only be replaced as a whole,
    /// by a replacement that keeps their placeholders.
    pub fn apply(&self, s: &ast::Str, changes: &[Change]) -> Option<ast::Str> {
        if changes.is_empty() {
            return None;
        }
        let base: usize = s.syntax().text_range().start().into();
        let original = s.syntax().to_string();
        let mut interps: Vec<(usize, usize)> = self
            .lines
            .iter()
            .flat_map(|l| &l.chars)
            .filter(|c| c.interp)
            .map(|c| (c.start, c.end))
            .collect();
        interps.dedup();

        let mut edits = Vec::new();
        for r in changes {
            let (a, exact_a) = self.source_offset(r.line, r.column, false)?;
            let (b, exact_b) = self.source_offset(r.end_line, r.end_column, true)?;
            if !exact_a || !exact_b || b < a {
                return None;
            }
            let covered: Vec<_> = interps
                .iter()
                .filter(|&&(s, e)| a < e && s < b)
                .copied()
                .collect();
            if covered.iter().any(|&(s, e)| s < a || e > b) {
                return None;
            }
            // Escape the replacement, putting the covered `${...}` back in
            // place of their placeholders.
            let pieces: Vec<&str> = r.replacement.split(PLACEHOLDER).collect();
            if pieces.len() != covered.len() + 1 {
                return None;
            }
            let mut source = self.escape(pieces[0]);
            for (piece, (s, e)) in pieces[1..].iter().zip(&covered) {
                source.push_str(&original[s - base..e - base]);
                source.push_str(&self.escape(piece));
            }
            let ta = self.text_offset(r.line, r.column)?;
            let tb = self.text_offset(r.end_line, r.end_column)?;
            edits.push((a - base, b - base, ta, tb, source, r.replacement.as_str()));
        }
        // Apply back to front; bail out on overlapping edits.
        edits.sort_by_key(|e| std::cmp::Reverse((e.0, e.1)));
        if edits.windows(2).any(|w| w[1].1 > w[0].0) {
            return None;
        }

        let mut src = original.clone();
        let mut expected = self.text.clone();
        for (a, b, ta, tb, source, replacement) in edits {
            src.replace_range(a..b, &source);
            expected.replace_range(ta..tb, replacement);
        }
        let parse = rnix::Root::parse(&src).ok().ok()?;
        let Some(ast::Expr::Str(fixed)) = parse.expr() else {
            return None;
        };
        (Script::new(&fixed).text == expected).then_some(fixed)
    }

    /// Escape shell text for insertion into this Nix string.
    pub fn escape(&self, s: &str) -> String {
        if self.indented {
            let nl = format!("\n{}", " ".repeat(self.indent));
            s.replace("''", "'''")
                .replace("${", "''${")
                .replace('\n', &nl)
        } else {
            s.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace("${", "\\${")
                .replace('\n', "\\n")
        }
    }
}

fn decode(text: &str, base: usize, indented: bool, out: &mut Vec<Item>) {
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let next_end = |it: &mut std::iter::Peekable<std::str::CharIndices>| {
            it.peek().map_or(text.len(), |(j, _)| *j)
        };
        let mut push = |c, end: usize, escaped| {
            out.push(Item::Char {
                c,
                src: Src {
                    start: base + i,
                    end: base + end,
                    interp: false,
                },
                escaped,
            });
        };
        if indented && c == '\'' && text[i..].starts_with("''") {
            it.next();
            match it.next() {
                Some((_, '\'')) => {
                    // `'''` -> `''`
                    let end = next_end(&mut it);
                    push('\'', end, true);
                    push('\'', end, true);
                }
                Some((_, '$')) => push('$', next_end(&mut it), true),
                Some((_, '\\')) => {
                    if let Some((_, e)) = it.next() {
                        let c = match e {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            e => e,
                        };
                        push(c, next_end(&mut it), true);
                    }
                }
                Some((j, e)) => {
                    push('\'', j, false);
                    push('\'', j, false);
                    push(e, next_end(&mut it), false);
                }
                None => {
                    push('\'', text.len(), false);
                    push('\'', text.len(), false);
                }
            }
        } else if !indented && c == '\\' {
            if let Some((_, e)) = it.next() {
                let c = match e {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    e => e,
                };
                push(c, next_end(&mut it), true);
            }
        } else {
            push(c, next_end(&mut it), false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rnix::ast::Expr;

    fn script(src: &str) -> Script {
        let Some(Expr::Str(s)) = rnix::Root::parse(src).tree().expr() else {
            panic!("not a string")
        };
        Script::new(&s)
    }

    #[test]
    fn renders_like_nix() {
        let s = script("''\n    echo $X\n      ls ${pkgs.hello} ''${Y} '''q\n  ''");
        assert_eq!(s.text, format!("echo $X\n  ls {PLACEHOLDER} ${{Y}} ''q\n"));
        assert_eq!(s.indent, 4);
        let s = script(r#""a\"b\${c} ${d}""#);
        assert_eq!(s.text, format!("a\"b${{c}} {PLACEHOLDER}"));
    }

    #[test]
    fn maps_positions() {
        let src = "''\n  echo $X ${y}\n''";
        let s = script(src);
        // `$X`
        let (o, exact) = s.source_offset(1, 6, false).unwrap();
        assert!(exact);
        assert_eq!(&src[o..o + 2], "$X");
        // inside the placeholder
        let (o, exact) = s.source_offset(1, 11, false).unwrap();
        assert!(!exact);
        assert_eq!(&src[o..o + 4], "${y}");
        // end of line
        let (o, exact) = s.source_offset(1, 9 + PLACEHOLDER.len(), true).unwrap();
        assert!(exact);
        assert_eq!(&src[o..=o], "\n");
    }
}
