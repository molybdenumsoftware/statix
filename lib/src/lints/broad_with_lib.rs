use crate::{
    Metadata, Report, Rule, Severity,
    utils::{has_local_value_binding, is_shadowed},
};
use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{Attr, AttrpathValue, Expr, With},
};
use rowan::ast::AstNode as _;

use super::optionals_string::{enclosing_parens, unparen};

/// ## What it does
/// Advises against the exact `meta = with lib; { ... };` shape when `lib`
/// is a visible conventional supplied parameter, not a local value definition.
///
/// ## Why is this bad?
/// A broad namespace can silently supply an unintended name. The reported
/// `lib.version` changelog failure is documented at
/// <https://github.com/NixOS/nixpkgs/issues/292468#issuecomment-2016454242>.
/// Nixpkgs removed broad meta scopes in
/// <https://github.com/NixOS/nixpkgs/pull/487414>.
/// This is configurable style advice, not proof that this expression is wrong:
/// AST inspection cannot establish the supplied library's identity or intent.
/// Narrow `with lib.maintainers; [ ... ]` scopes are not diagnosed. No fix is
/// offered because qualifying names requires resolving their actual owners.
///
/// ## Example
/// ```nix
/// { lib }: { meta = with lib; { license = licenses.mit; }; }
/// ```
#[lint(
    name = "broad_with_lib",
    note = "Broad library scope in meta",
    code = 30,
    match_with = SyntaxKind::NODE_WITH
)]
struct BroadWithLib;

impl Rule for BroadWithLib {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let with = With::cast(node.clone())?;
        let Expr::Ident(lib) = unparen(with.namespace()?)? else {
            return None;
        };
        if lib.ident_token()?.text() != "lib"
            || !is_shadowed(lib.syntax(), "lib")
            || has_local_value_binding(lib.syntax(), "lib")
            || !matches!(unparen(with.body()?)?, Expr::AttrSet(_))
        {
            return None;
        }
        let expression = enclosing_parens(node.clone());
        let assignment = AttrpathValue::cast(expression.parent()?)?;
        if assignment.value()?.syntax() != &expression {
            return None;
        }
        let path = assignment.attrpath()?;
        let mut attrs = path.attrs();
        let Some(Attr::Ident(meta)) = attrs.next() else {
            return None;
        };
        if meta.ident_token()?.text() != "meta" || attrs.next().is_some() {
            return None;
        }
        Some(self.report().severity(Severity::Hint).diagnostic(
            node.text_range(),
            "Consider explicit library references in meta; a broad `with lib` can silently resolve unintended names",
        ))
    }
}
