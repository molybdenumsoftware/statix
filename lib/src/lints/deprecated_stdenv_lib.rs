use crate::{Metadata, Report, Rule, Suggestion, make, utils};
use macros::lint;
use rnix::{
    SyntaxElement, SyntaxKind, TextRange,
    ast::{Attr, Expr, Select},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Warns about the removed Nixpkgs `stdenv.lib` library alias.
///
/// ## Why is this bad?
/// Nixpkgs uses an explicit `lib` argument as its canonical library binding.
/// A local fix is offered only when `lib` is already supplied as an argument;
/// adding required arguments can break manual callers. Locally constructed
/// `stdenv`/`lib` values and package `.lib` outputs are not rewritten.
/// See <https://github.com/NixOS/nixpkgs/issues/108938>.
#[lint(
    name = "deprecated_stdenv_lib",
    note = "Use the canonical Nixpkgs lib argument",
    code = 26,
    match_with = SyntaxKind::NODE_SELECT
)]
struct DeprecatedStdenvLib;

impl Rule for DeprecatedStdenvLib {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let select = Select::cast(node.as_node()?.clone())?;
        if select.or_token().is_some() {
            return None;
        }
        let Expr::Ident(base) = select.expr()? else {
            return None;
        };
        if base.ident_token()?.text() != "stdenv"
            || utils::has_local_value_binding(select.syntax(), "stdenv")
        {
            return None;
        }
        let Attr::Ident(first) = select.attrpath()?.attrs().next()? else {
            return None;
        };
        if first.ident_token()?.text() != "lib" {
            return None;
        }
        let at = TextRange::new(
            base.syntax().text_range().start(),
            first.syntax().text_range().end(),
        );
        let report = self.report();
        let can_fix = utils::is_shadowed(select.syntax(), "lib")
            && !utils::has_local_value_binding(select.syntax(), "lib")
            && !select.syntax().descendants_with_tokens().any(|element| {
                element.kind() == SyntaxKind::TOKEN_COMMENT
                    && at.contains_range(element.text_range())
            });
        Some(if can_fix {
            report.suggest(
                at,
                "Use the existing lib argument",
                Suggestion::with_replacement(at, make::ident("lib").syntax().clone()),
            )
        } else {
            report.diagnostic(
                at,
                "Use an explicit lib argument; update manual callers before adding it",
            )
        })
    }
}
