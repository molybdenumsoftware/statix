use super::optionals_string::unparen;
use crate::{Metadata, Report, Rule, Severity, Suggestion, make, utils};
use macros::lint;
use rnix::{
    SyntaxElement, SyntaxKind,
    ast::{Apply, AstToken, Attr, AttrSet, Entry, Expr, HasEntry, InterpolPart},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Prefers the algorithm-independent `hash` field for literal SRI hashes in
/// conventional Nixpkgs `fetchFromGitHub` calls.
///
/// ## Why use this?
/// A key-only rename keeps the SHA-256 SRI payload unchanged. This is a
/// modernization hint, not a claim that all `sha256` attributes are deprecated.
/// Local wrappers, recursive sets, multiple algorithm fields, existing hashes,
/// legacy encodings, and dynamic fields are excluded.
/// See <https://github.com/NixOS/nixpkgs/pull/558623>.
#[lint(
    name = "fetcher_hash_field",
    note = "Prefer hash for a fetcher SRI value",
    code = 27,
    match_with = SyntaxKind::NODE_APPLY
)]
struct FetcherHashField;

impl Rule for FetcherHashField {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let apply = Apply::cast(node.as_node()?.clone())?;
        let Expr::Ident(callee) = unparen(apply.lambda()?)? else {
            return None;
        };
        if callee.ident_token()?.text() != "fetchFromGitHub"
            || utils::has_local_value_binding(callee.syntax(), "fetchFromGitHub")
        {
            return None;
        }
        let Expr::AttrSet(attrs) = unparen(apply.argument()?)? else {
            return None;
        };
        if attrs.rec_token().is_some() {
            return None;
        }
        let key = sri_key(&attrs)?;
        Some(self.report().severity(Severity::Hint).suggest(
            key.syntax().text_range(),
            "Keep the SRI value and use the hash field",
            Suggestion::with_replacement(
                key.syntax().text_range(),
                make::ident("hash").syntax().clone(),
            ),
        ))
    }
}

fn sri_key(attrs: &AttrSet) -> Option<rnix::ast::Ident> {
    let mut selected = None;
    for entry in attrs.entries() {
        let Entry::AttrpathValue(entry) = entry else {
            return None;
        };
        let mut path = entry.attrpath()?.attrs();
        let Attr::Ident(key) = path.next()? else {
            return None;
        };
        let token = key.ident_token()?;
        if matches!(token.text(), "hash" | "sha1" | "sha512" | "md5") {
            return None;
        }
        if token.text() != "sha256" {
            continue;
        }
        if selected.is_some() || path.next().is_some() {
            return None;
        }
        let Expr::Str(value) = entry.value()? else {
            return None;
        };
        let mut parts = value.parts();
        let Some(InterpolPart::Literal(literal)) = parts.next() else {
            return None;
        };
        if parts.next().is_some() {
            return None;
        }
        let encoded = literal.syntax().text().strip_prefix("sha256-")?;
        if encoded.len() != 44
            || !encoded.ends_with('=')
            || !encoded.as_bytes()[..43]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'+' | b'/'))
        {
            return None;
        }
        selected = Some(key);
    }
    selected
}
