use crate::{Metadata, Report, Rule, Severity, utils::is_shadowed};
use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind, SyntaxNode,
    ast::{self, AstToken, Attr, Expr, HasEntry, InterpolPart},
};
use rowan::ast::AstNode as _;

use super::optionals_string::unparen;

/// ## What it does
/// Finds a body variable also present as a static attribute of a literal
/// `with` namespace and lexically bound outside that `with` expression.
///
/// ## Why is this bad?
/// Lexical bindings win over `with` attributes, as illustrated at
/// <https://discourse.nixos.org/t/scoping-of-with-expressions/23484/5>.
/// This is legal and may be intentional; the Hint explains existing resolution
/// rather than claiming an undefined variable or offering a semantics-changing
/// qualification/removal. Opaque namespaces cannot prove a collision and are
/// excluded. Nested `with` scopes, string expressions and inherit clauses are
/// conservatively excluded; labels are never variable references. Inner lexical
/// bindings are intentional and do not demonstrate an outer capture.
///
/// ## Example
/// ```nix
/// let foo = "local"; in with { foo = "package"; bar = "other"; }; [ foo bar ]
/// ```
#[lint(
    name = "with_lexical_collision",
    note = "Lexical binding wins over a with namespace attribute",
    code = 31,
    match_with = SyntaxKind::NODE_WITH
)]
struct WithLexicalCollision;

impl Rule for WithLexicalCollision {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let with = ast::With::cast(node.clone())?;
        let Expr::AttrSet(namespace) = unparen(with.namespace()?)? else {
            return None;
        };
        let body = with.body()?;
        let reference = collision(body.syntax(), node, &namespace)?;
        Some(self.report().severity(Severity::Hint).diagnostic(
            reference.text_range(),
            "The outer lexical binding wins over this literal `with` namespace attribute; this is legal and may be intentional",
        ))
    }
}

fn collision(node: &SyntaxNode, with: &SyntaxNode, namespace: &ast::AttrSet) -> Option<SyntaxNode> {
    // Do not descend into labels, strings, inherits or another with scope.
    if ast::Attrpath::cast(node.clone()).is_some()
        || ast::Str::cast(node.clone()).is_some()
        || ast::Inherit::cast(node.clone()).is_some()
        || ast::With::cast(node.clone()).is_some()
        || ast::IdentParam::cast(node.clone()).is_some()
        || ast::Pattern::cast(node.clone()).is_some()
    {
        return None;
    }
    if let Some(ident) = ast::Ident::cast(node.clone()) {
        let token = ident.ident_token()?;
        let name = token.text();
        if binds_entries(namespace, name)
            && is_shadowed(with, name)
            && !inner_binding(node, with, name)
        {
            return Some(node.clone());
        }
        return None;
    }
    node.children()
        .find_map(|child| collision(&child, with, namespace))
}

fn inner_binding(node: &SyntaxNode, with: &SyntaxNode, name: &str) -> bool {
    node.ancestors()
        .skip(1)
        .take_while(|scope| scope != with)
        .any(|scope| {
            if let Some(lambda) = ast::Lambda::cast(scope.clone()) {
                return lambda.param().is_some_and(|param| match param {
                    ast::Param::IdentParam(param) => {
                        param.ident().is_some_and(|id| ident_matches(&id, name))
                    }
                    ast::Param::Pattern(pattern) => {
                        pattern
                            .pat_entries()
                            .any(|entry| entry.ident().is_some_and(|id| ident_matches(&id, name)))
                            || pattern
                                .pat_bind()
                                .and_then(|bind| bind.ident())
                                .is_some_and(|id| ident_matches(&id, name))
                    }
                });
            }
            if let Some(scope) = ast::LetIn::cast(scope.clone()) {
                return inner_entries_bind(&scope, name);
            }
            if let Some(scope) = ast::LegacyLet::cast(scope.clone()) {
                return inner_entries_bind(&scope, name);
            }
            ast::AttrSet::cast(scope).is_some_and(|scope| {
                scope.rec_token().is_some() && inner_entries_bind(&scope, name)
            })
        })
}

fn binds_entries(scope: &impl HasEntry, name: &str) -> bool {
    scope.entries().any(|entry| match entry {
        ast::Entry::AttrpathValue(entry) => entry
            .attrpath()
            .and_then(|path| path.attrs().next())
            .is_some_and(|attr| attr_matches(&attr, name)),
        // Unqualified inherit brings in the very same outer binding, not a collision.
        ast::Entry::Inherit(inherit) => {
            inherit.from().is_some() && inherit.attrs().any(|attr| attr_matches(&attr, name))
        }
    })
}

// A quoted or dynamic inner binding may require evaluation to identify.
// Skip that ambiguous scope rather than misattribute a reference to the outer owner.
fn inner_entries_bind(scope: &impl HasEntry, name: &str) -> bool {
    scope.entries().any(|entry| match entry {
        ast::Entry::AttrpathValue(entry) => entry
            .attrpath()
            .and_then(|path| path.attrs().next())
            .is_some_and(|attr| !matches!(&attr, Attr::Ident(_)) || attr_matches(&attr, name)),
        ast::Entry::Inherit(inherit) => inherit
            .attrs()
            .any(|attr| !matches!(&attr, Attr::Ident(_)) || attr_matches(&attr, name)),
    })
}

fn ident_matches(ident: &ast::Ident, name: &str) -> bool {
    ident
        .ident_token()
        .is_some_and(|token| token.text() == name)
}

fn attr_matches(attr: &Attr, name: &str) -> bool {
    match attr {
        Attr::Ident(ident) => ident_matches(ident, name),
        Attr::Str(string) => {
            let mut parts = string.parts();
            matches!((parts.next(), parts.next()), (Some(InterpolPart::Literal(literal)), None) if literal.syntax().text() == name)
        }
        Attr::Dynamic(_) => false,
    }
}
