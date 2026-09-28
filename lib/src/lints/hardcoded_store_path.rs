use crate::{Metadata, Report, Rule};

use macros::lint;
use rnix::{NodeOrToken, SyntaxElement, SyntaxKind, TextRange, TextSize};

/// ## What it does
/// Checks for hardcoded `/nix/store/<hash>-<name>` paths.
///
/// ## Why is this bad?
/// Store paths change whenever the package or its dependencies change, so
/// the path breaks on the next nixpkgs update or garbage collection, and the
/// dependency is invisible to Nix.
///
/// ## Example
///
/// ```nix
/// "/nix/store/q2l6gv5cdyx2ayx6frgz2rmd0mp4q6sw-hello-2.12.1/bin/hello"
/// ```
///
/// Interpolate the package instead:
///
/// ```nix
/// "${pkgs.hello}/bin/hello"
/// ```
#[lint(
    name = "hardcoded_store_path",
    note = "Found hardcoded Nix store path",
    code = 26,
    match_with = [SyntaxKind::TOKEN_STRING_CONTENT, SyntaxKind::TOKEN_PATH_ABS]
)]
struct HardcodedStorePath;

const STORE: &str = "/nix/store/";
const HASH_LEN: usize = 32;

fn is_nix_base32(c: u8) -> bool {
    // Nix base32 omits e, o, u and t.
    c.is_ascii_digit() || (c.is_ascii_lowercase() && !b"eout".contains(&c))
}

impl Rule for HardcodedStorePath {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Token(token) = node else {
            return None;
        };
        let text = token.text();
        let start = token.text_range().start();

        let mut report = self.report();
        for (i, _) in text.match_indices(STORE) {
            let rest = &text.as_bytes()[i + STORE.len()..];
            let is_store_path = rest.len() > HASH_LEN
                && rest[..HASH_LEN].iter().all(|&c| is_nix_base32(c))
                && rest[HASH_LEN] == b'-';
            if !is_store_path {
                continue;
            }
            let len = text[i..]
                .find(|c: char| c.is_whitespace() || "\"'`;:)".contains(c))
                .unwrap_or(text.len() - i);
            let at = TextRange::at(
                start + TextSize::try_from(i).ok()?,
                TextSize::try_from(len).ok()?,
            );
            report = report.diagnostic(
                at,
                "Hardcoded store path, interpolate the package instead (e.g. `${pkgs.hello}`)",
            );
        }
        (!report.diagnostics.is_empty()).then_some(report)
    }
}
