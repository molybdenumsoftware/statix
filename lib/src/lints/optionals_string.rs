use crate::{Metadata, Report, Rule, Suggestion, make, utils::has_local_value_binding};

use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode,
    ast::{Apply, Attr, BinOp, BinOpKind, Expr, Ident, Paren},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Checks for `lib.optionals` returning a list in a string concatenation.
///
/// ## Why is this bad?
/// Nixpkgs `lib.optionals` produces a list, not a string. Use `lib.optionalString`
/// to conditionally include a string. A supplied `lib` parameter is assumed to
/// follow the Nixpkgs convention; locally defined `lib` values are excluded.
///
/// ## Example
/// ```nix
/// "start" + lib.optionals enabled "extra"
/// ```
/// Use `lib.optionalString enabled "extra"` instead.
#[lint(
    name = "optionals_string",
    note = "List helper used in a string concatenation",
    code = 24,
    match_with = SyntaxKind::NODE_APPLY
)]
struct OptionalsString;

impl Rule for OptionalsString {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let apply = Apply::cast(node.clone())?;
        if !matches!(unparen(apply.argument()?)?, Expr::Str(_)) {
            return None;
        }
        let Expr::Apply(first) = unparen(apply.lambda()?)? else {
            return None;
        };
        first.argument()?;
        let attr = lib_member(first.lambda()?, "optionals")?;
        let operand = enclosing_parens(node.clone());
        let bin = BinOp::cast(operand.parent()?)?;
        if bin.operator()? != BinOpKind::Add {
            return None;
        }
        let sibling = if bin.lhs()?.syntax() == &operand {
            bin.rhs()?
        } else if bin.rhs()?.syntax() == &operand {
            bin.lhs()?
        } else {
            return None;
        };
        if !matches!(unparen(sibling)?, Expr::Str(_)) {
            return None;
        }
        Some(self.report().suggest(
            node.text_range(),
            "Use `lib.optionalString` to conditionally include a string",
            Suggestion::with_replacement(
                attr.syntax().text_range(),
                make::ident("optionalString").syntax().clone(),
            ),
        ))
    }
}

pub(super) fn unparen(mut expr: Expr) -> Option<Expr> {
    while let Expr::Paren(paren) = expr {
        expr = paren.expr()?;
    }
    Some(expr)
}

pub(super) fn enclosing_parens(mut node: SyntaxNode) -> SyntaxNode {
    while let Some(parent) = node.parent().and_then(Paren::cast) {
        node = parent.syntax().clone();
    }
    node
}

pub(super) fn lib_member(expr: Expr, member: &str) -> Option<Ident> {
    let Expr::Select(select) = unparen(expr)? else {
        return None;
    };
    if select.default_expr().is_some() || select.or_token().is_some() {
        return None;
    }
    let Expr::Ident(base) = unparen(select.expr()?)? else {
        return None;
    };
    if base.ident_token()?.text() != "lib" || has_local_value_binding(select.syntax(), "lib") {
        return None;
    }
    let path = select.attrpath()?;
    let mut attrs = path.attrs();
    let Attr::Ident(attr) = attrs.next()? else {
        return None;
    };
    if attrs.next().is_some() || attr.ident_token()?.text() != member {
        return None;
    }
    Some(attr)
}
