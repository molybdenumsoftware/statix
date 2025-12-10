use crate::{Metadata, Report, Rule};

use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode,
    ast::{BinOp, Ident},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Checks for equality comparison between two boolean literals.
///
/// ## Example
///
/// ```nix
/// false == false
/// ```
#[lint(
    name = "bool_comparison",
    note = "Equality comparison between boolean literals",
    code = 1,
    match_with = SyntaxKind::NODE_BIN_OP
)]
struct BoolComparison;

impl Rule for BoolComparison {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let bin_expr = BinOp::cast(node.clone())?;
        let (lhs, rhs) = (bin_expr.lhs()?, bin_expr.rhs()?);

        if boolean_ident(lhs.syntax()).is_none() || boolean_ident(rhs.syntax()).is_none() {
            return None;
        }

        let at = node.text_range();
        Some(
            self.report()
                .diagnostic(at, "Equality comparison between boolean literals"),
        )
    }
}

enum NixBoolean {
    True,
    False,
}

impl std::fmt::Display for NixBoolean {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::True => "true",
            Self::False => "false",
        };
        write!(f, "{s}")
    }
}

// not entirely accurate, underhanded nix programmers might write `true = false`
fn boolean_ident(node: &SyntaxNode) -> Option<NixBoolean> {
    Ident::cast(node.clone()).and_then(|ident_expr| match ident_expr.to_string().as_str() {
        "true" => Some(NixBoolean::True),
        "false" => Some(NixBoolean::False),
        _ => None,
    })
}
