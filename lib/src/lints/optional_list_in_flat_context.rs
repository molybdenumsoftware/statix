use crate::{Metadata, Report, Rule, Suggestion, make};

use super::optionals_string::{enclosing_parens, lib_member, unparen};
use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode,
    ast::{Apply, BinOp, BinOpKind, Expr},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Checks for `lib.optional` wrapping a literal list passed to `lib.makeBinPath`.
///
/// ## Why is this bad?
/// Nixpkgs `lib.optional` adds its value as one list element, creating a nested
/// list here. `lib.optionals` conditionally includes the list's elements instead.
/// Only direct arguments and list concatenations flowing into `lib.makeBinPath`
/// are checked; intentional nested lists and other consumers are left alone.
/// A supplied `lib` parameter is assumed to follow the Nixpkgs convention.
///
/// ## Example
/// ```nix
/// lib.makeBinPath (lib.optional enabled [ package ])
/// ```
/// Use `lib.optionals enabled [ package ]` instead.
#[lint(
    name = "optional_list_in_flat_context",
    note = "Nested list passed to a flat list consumer",
    code = 25,
    match_with = SyntaxKind::NODE_APPLY
)]
struct OptionalListInFlatContext;

impl Rule for OptionalListInFlatContext {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let apply = Apply::cast(node.clone())?;
        if !matches!(unparen(apply.argument()?)?, Expr::List(_)) {
            return None;
        }
        let Expr::Apply(first) = unparen(apply.lambda()?)? else {
            return None;
        };
        first.argument()?;
        let attr = lib_member(first.lambda()?, "optional")?;
        if !flows_to_make_bin_path(node.clone()) {
            return None;
        }
        Some(self.report().suggest(
            node.text_range(),
            "Use `lib.optionals` to include list elements in `lib.makeBinPath`",
            Suggestion::with_replacement(
                attr.syntax().text_range(),
                make::ident("optionals").syntax().clone(),
            ),
        ))
    }
}

fn flows_to_make_bin_path(mut node: SyntaxNode) -> bool {
    loop {
        node = enclosing_parens(node);
        let Some(parent) = node.parent() else {
            return false;
        };
        if let Some(bin) = BinOp::cast(parent.clone()) {
            if bin.operator() != Some(BinOpKind::Concat) {
                return false;
            }
            node = parent;
            continue;
        }
        let Some(consumer) = Apply::cast(parent) else {
            return false;
        };
        let Some(argument) = consumer.argument() else {
            return false;
        };
        if argument.syntax() != &node {
            return false;
        }
        return consumer
            .lambda()
            .and_then(|callee| lib_member(callee, "makeBinPath"))
            .is_some();
    }
}
