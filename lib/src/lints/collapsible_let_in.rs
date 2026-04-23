use crate::{Metadata, Report, Rule, Suggestion};

use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind, TextRange,
    ast::{Attr, Entry, Expr, HasEntry, LetIn},
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

fn defined_names(let_expr: &LetIn) -> Vec<String> {
    let_expr
        .entries()
        .flat_map(|entry| match entry {
            Entry::AttrpathValue(b) => {
                let Some(attrpath) = b.attrpath() else {
                    return vec![];
                };
                let Some(Attr::Ident(ident)) = attrpath.attrs().next() else {
                    return vec![];
                };
                let Some(token) = ident.ident_token() else {
                    return vec![];
                };
                vec![token.text().to_string()]
            }
            Entry::Inherit(i) => i
                .attrs()
                .filter_map(|attr| {
                    let Attr::Ident(ident) = attr else {
                        return None;
                    };
                    let token = ident.ident_token()?;
                    Some(token.text().to_string())
                })
                .collect::<Vec<_>>(),
        })
        .collect()
}

fn referenced_names(expr: &Expr) -> Vec<String> {
    expr.syntax()
        .descendants()
        .filter_map(|n| {
            // Exclude idents that are attribute path selectors (e.g. the `b`
            // in `a.b`), which are not variable references.
            if n.parent()
                .is_some_and(|p| p.kind() == SyntaxKind::NODE_ATTRPATH)
            {
                return None;
            }
            let Expr::Ident(ident) = Expr::cast(n)? else {
                return None;
            };
            let token = ident.ident_token()?;
            Some(token.text().to_string())
        })
        .collect()
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
        let outer_value_names: Vec<String> = let_in_expr
            .attrpath_values()
            .filter_map(|b| b.value())
            .flat_map(|v| referenced_names(&v))
            .collect();

        if inner_names
            .iter()
            .any(|n| outer_names.contains(n) || outer_value_names.contains(n))
        {
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
