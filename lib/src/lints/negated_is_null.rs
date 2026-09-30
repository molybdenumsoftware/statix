use crate::{Metadata, Report, Rule, Severity, Suggestion, make, utils::is_shadowed};

use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{Attr, BinOp, BinOpKind, Expr, UnaryOp, UnaryOpKind},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Suggests a direct null comparison instead of a negated builtin `isNull` call.
///
/// ## Why use this?
/// `value != null` is an optional readability improvement. `isNull` remains a
/// valid builtin, including when passed as a higher-order function.
///
/// ## Example
/// ```nix
/// !(builtins.isNull value)
/// ```
///
/// Can be written as:
/// ```nix
/// value != null
/// ```
#[lint(
    name = "negated_is_null",
    note = "A direct null comparison may be easier to read",
    code = 23,
    match_with = SyntaxKind::NODE_UNARY_OP
)]
struct NegatedIsNull;

impl Rule for NegatedIsNull {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let unary = UnaryOp::cast(node.clone())?;
        if unary.operator() != Some(UnaryOpKind::Invert) || is_shadowed(node, "null") {
            return None;
        }
        let Expr::Apply(apply) = unparenthesize(unary.expr()?)? else {
            return None;
        };
        match unparenthesize(apply.lambda()?)? {
            Expr::Ident(ident)
                if ident.ident_token()?.text() == "isNull"
                    && !is_shadowed(ident.syntax(), "isNull") => {}
            Expr::Select(select) if select.default_expr().is_none() => {
                let Expr::Ident(base) = unparenthesize(select.expr()?)? else {
                    return None;
                };
                let mut attrs = select.attrpath()?.attrs();
                let Attr::Ident(attr) = attrs.next()? else {
                    return None;
                };
                if base.ident_token()?.text() != "builtins"
                    || attr.ident_token()?.text() != "isNull"
                    || attrs.next().is_some()
                    || is_shadowed(base.syntax(), "builtins")
                {
                    return None;
                }
            }
            _ => return None,
        }

        let value = apply.argument()?;
        if node.descendants_with_tokens().any(|element| {
            element.kind() == SyntaxKind::TOKEN_COMMENT
                && !value
                    .syntax()
                    .text_range()
                    .contains_range(element.text_range())
        }) {
            return Some(self.report().severity(Severity::Hint).diagnostic(
                node.text_range(),
                "Consider a direct null comparison; preserve the surrounding comments",
            ));
        }
        // A selection default absorbs the expression to its right. Other
        // non-atomic expressions likewise need their original grouping.
        let value_node = match &value {
            Expr::Select(select) if select.default_expr().is_some() => {
                make::parenthesize(value.syntax()).syntax().clone()
            }
            Expr::BinOp(_)
            | Expr::Lambda(_)
            | Expr::LetIn(_)
            | Expr::IfElse(_)
            | Expr::Assert(_)
            | Expr::With(_) => make::parenthesize(value.syntax()).syntax().clone(),
            _ => value.syntax().clone(),
        };
        let null = make::ident("null");
        let comparison = make::binary(&value_node, "!=", null.syntax());
        // The replacement binds less tightly than the original unary operation.
        let needs_grouping = node.parent().is_some_and(|parent| {
            if let Some(binary) = BinOp::cast(parent.clone()) {
                !matches!(
                    binary.operator(),
                    Some(BinOpKind::And | BinOpKind::Or | BinOpKind::Implication)
                )
            } else {
                matches!(
                    parent.kind(),
                    SyntaxKind::NODE_APPLY
                        | SyntaxKind::NODE_UNARY_OP
                        | SyntaxKind::NODE_SELECT
                        | SyntaxKind::NODE_HAS_ATTR
                        | SyntaxKind::NODE_LIST
                )
            }
        });
        let replacement = if needs_grouping {
            make::parenthesize(comparison.syntax()).syntax().clone()
        } else {
            comparison.syntax().clone()
        };
        let at = node.text_range();
        Some(self.report().severity(Severity::Hint).suggest(
            at,
            "Consider `value != null` instead of a negated `isNull` call",
            Suggestion::with_replacement(at, replacement),
        ))
    }
}

fn unparenthesize(mut expr: Expr) -> Option<Expr> {
    while let Expr::Paren(paren) = expr {
        expr = paren.expr()?;
    }
    Some(expr)
}
