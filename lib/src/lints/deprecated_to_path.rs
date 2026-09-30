use crate::{Metadata, Report, Rule, utils::is_shadowed};

use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{Apply, Attr, Expr},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Checks for usage of the `toPath` function.
///
/// ## Why is this bad?
/// `toPath` is deprecated.
///
/// ## Example
///
/// ```nix
/// builtins.toPath "/path"
/// ```
///
/// Try these instead:
///
/// ```nix
/// # to convert the string to an absolute path:
/// /. + "/path"
/// # => /abc
///
/// # to convert the string to a path relative to the current directory:
/// ./. + "/bin"
/// # => /home/np/statix/bin
/// ```
#[lint(
    name = "deprecated_to_path",
    note = "Found usage of deprecated builtin toPath",
    code = 17,
    match_with = SyntaxKind::NODE_APPLY
)]
struct DeprecatedToPath;

impl Rule for DeprecatedToPath {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let apply = Apply::cast(node.clone())?;
        let mut callee = apply.lambda()?;
        while let Expr::Paren(paren) = callee {
            callee = paren.expr()?;
        }
        let lambda_path = match callee {
            Expr::Ident(ident)
                if ident.ident_token()?.text() == "toPath"
                    && !is_shadowed(ident.syntax(), "toPath") =>
            {
                "toPath"
            }
            Expr::Select(select) if select.default_expr().is_none() => {
                let mut base = select.expr()?;
                while let Expr::Paren(paren) = base {
                    base = paren.expr()?;
                }
                let Expr::Ident(ident) = base else {
                    return None;
                };
                let mut attrs = select.attrpath()?.attrs();
                let Attr::Ident(attr) = attrs.next()? else {
                    return None;
                };
                if ident.ident_token()?.text() != "builtins"
                    || attr.ident_token()?.text() != "toPath"
                    || attrs.next().is_some()
                    || is_shadowed(ident.syntax(), "builtins")
                {
                    return None;
                }
                "builtins.toPath"
            }
            _ => return None,
        };
        let message = format!(
            "`{lambda_path}` is deprecated, see `:doc builtins.toPath` within the REPL for more"
        );
        Some(self.report().diagnostic(node.text_range(), message))
    }
}
