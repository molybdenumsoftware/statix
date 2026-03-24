use crate::{Metadata, Report, Rule, Suggestion};

use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind, TextRange,
    ast::{Attr, Entry, Expr, HasEntry, Ident, LetIn},
};
use rowan::{Direction, ast::AstNode as _};

/// ## What it does
/// Checks for `let-in` expressions whose body is another `let-in`
/// expression.
///
/// ## Why is this bad?
/// Unnecessary code, the `let-in` expressions can be merged.
///
/// ## Example
///
/// ```nix
/// let
///   a = 2;
/// in
/// let
///   b = 3;
/// in
///   a + b
/// ```
///
/// Merge both `let-in` expressions:
///
/// ```nix
/// let
///   a = 2;
///   b = 3;
/// in
///   a + b
/// ```
#[lint(
    name = "collapsible_let_in",
    note = "These let-in expressions are collapsible",
    code = 6,
    match_with = SyntaxKind::NODE_LET_IN
)]
struct CollapsibleLetIn;

fn defined_names(let_expr: &LetIn) -> Vec<Ident> {
    let_expr
        .entries()
        .flat_map(|entry| match entry {
            Entry::AttrpathValue(b) => {
                let Some(attrpath) = b.attrpath() else {
                    return vec![];
                };
                attrpath
                    .attrs()
                    .filter_map(|attr| {
                        if let Attr::Ident(ident) = attr {
                            Some(ident)
                        } else {
                            None
                        }
                    })
                    .take(1)
                    .collect::<Vec<_>>()
            }
            Entry::Inherit(i) => i
                .attrs()
                .filter_map(|attr| {
                    if let Attr::Ident(ident) = attr {
                        Some(ident)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>(),
        })
        .collect()
}

fn value_ident_names<T: HasEntry>(let_expr: &T) -> Vec<Ident> {
    let_expr
        .attrpath_values()
        .filter_map(|b| b.value())
        .flat_map(|v| {
            v.syntax()
                .descendants()
                .filter(|n| {
                    n.parent()
                        .is_none_or(|p| p.kind() != SyntaxKind::NODE_ATTRPATH)
                })
                .filter_map(Expr::cast)
                .filter_map(|expr| {
                    if let Expr::Ident(ident) = expr {
                        Some(ident)
                    } else {
                        None
                    }
                })
        })
        .collect()
}

fn ident_eq(a: &Ident, b: &Ident) -> bool {
    a.ident_token().map(|t| t.text().to_string()) == b.ident_token().map(|t| t.text().to_string())
}

impl Rule for CollapsibleLetIn {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };

        let let_in_expr = LetIn::cast(node.clone())?;
        let body = let_in_expr.body()?;

        let Expr::LetIn(inner_let) = &body else {
            return None;
        };

        let outer_names = defined_names(&let_in_expr);
        let inner_names = defined_names(inner_let);
        let outer_value_names = value_ident_names(&let_in_expr);

        if inner_names.iter().any(|n| {
            outer_names.iter().any(|o| ident_eq(o, n))
                || outer_value_names.iter().any(|o| ident_eq(o, n))
        }) {
            return None;
        }

        let first_annotation = node.text_range();
        let first_message = "This `let in` expression contains a nested `let in` expression";

        let second_annotation = body.syntax().text_range();
        let second_message = "This `let in` expression is nested";

        let replacement_at = {
            let start = body
                .syntax()
                .siblings_with_tokens(Direction::Prev)
                .find(|elem| elem.kind() == SyntaxKind::TOKEN_IN)?
                .text_range()
                .start();
            let end = body
                .syntax()
                .descendants_with_tokens()
                .find(|elem| elem.kind() == SyntaxKind::TOKEN_LET)?
                .text_range()
                .end();
            TextRange::new(start, end)
        };

        Some(
            self.report()
                .diagnostic(first_annotation, first_message)
                .suggest(
                    second_annotation,
                    second_message,
                    Suggestion::with_empty(replacement_at),
                ),
        )
    }
}
