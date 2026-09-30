use crate::{Metadata, Report, Rule, Suggestion};
use macros::lint;
use rnix::{NodeOrToken, SyntaxElement, SyntaxKind, TextRange, TextSize};

/// ## What it does
/// Finds ineffective backslash-space escapes in ordinary quoted Nix strings.
///
/// ## Why is this bad?
/// `"Application\ Support"` has the same value as `"Application Support"`.
/// The backslash suggests shell escaping that Nix does not preserve, and Lix
/// warns about this spelling. Escaped backslashes and indented strings have
/// different semantics and are not changed.
///
/// ## Example
/// ```nix
/// "Library/Application\ Support/go"
/// ```
/// Use `"Library/Application Support/go"` instead.
/// See <https://github.com/nix-community/home-manager/pull/8916>.
#[lint(
    name = "ineffective_string_escape",
    note = "Ineffective backslash-space escape",
    code = 22,
    match_with = SyntaxKind::TOKEN_STRING_CONTENT
)]
struct IneffectiveStringEscape;

impl Rule for IneffectiveStringEscape {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Token(token) = node else {
            return None;
        };
        let parent = token.parent()?;
        let first = parent.first_token()?;
        let last = parent.last_token()?;
        if first.kind() != SyntaxKind::TOKEN_STRING_START
            || first.text() != "\""
            || last.kind() != SyntaxKind::TOKEN_STRING_END
            || last.text() != "\""
        {
            return None;
        }
        let bytes = token.text().as_bytes();
        let mut offset = 0;
        while offset + 1 < bytes.len() {
            if bytes[offset] == b'\\' {
                if bytes[offset + 1] == b' ' {
                    let start = token.text_range().start() + TextSize::try_from(offset).ok()?;
                    let at = TextRange::new(start, start + TextSize::from(1));
                    // One deletion per pass avoids shifting multiple spans in a report.
                    return Some(self.report().suggest(
                        at,
                        "Remove the backslash; it does not escape the space in Nix",
                        Suggestion::with_empty(at),
                    ));
                }
                offset += 2;
            } else {
                offset += 1;
            }
        }
        None
    }
}
