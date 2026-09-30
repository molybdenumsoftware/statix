use rnix::{
    SyntaxKind, SyntaxNode, TextRange,
    ast::{self, AstToken, HasEntry, InterpolPart},
};
use rowan::ast::AstNode as _;

pub fn with_preceeding_whitespace(node: &SyntaxNode) -> TextRange {
    let start = node.prev_sibling_or_token().map_or_else(
        || node.text_range().start(),
        |t| {
            if t.kind() == SyntaxKind::TOKEN_WHITESPACE {
                t.text_range().start()
            } else {
                t.text_range().end()
            }
        },
    );
    let end = node.text_range().end();
    TextRange::new(start, end)
}

/// Whether the closest visible lexical binding overrides a builtin.
pub fn is_shadowed(node: &SyntaxNode, name: &str) -> bool {
    lexical_binding(node, name).is_some()
}

/// Whether a conventional library name resolves to a local value definition.
///
/// Lambda parameters are accepted as conventional inputs. An unresolved name
/// supplied by `with` is ambiguous and is conservatively treated as local.
pub fn has_local_value_binding(node: &SyntaxNode, name: &str) -> bool {
    match lexical_binding(node, name) {
        Some(Binding::Parameter) => false,
        Some(Binding::Value) => true,
        None => node.ancestors().any(|ancestor| {
            ast::With::cast(ancestor).is_some_and(|with| {
                with.body().is_some_and(|body| {
                    body.syntax().text_range().contains_range(node.text_range())
                })
            })
        }),
    }
}
enum Binding {
    Parameter,
    Value,
}

fn lexical_binding(node: &SyntaxNode, name: &str) -> Option<Binding> {
    let mut child = node.clone();
    for ancestor in node.ancestors().skip(1) {
        if let Some(lambda) = ast::Lambda::cast(ancestor.clone()) {
            let bound = match lambda.param()? {
                ast::Param::IdentParam(param) => param
                    .ident()
                    .is_some_and(|ident| ident_matches(&ident, name)),
                ast::Param::Pattern(pattern) => {
                    pattern.pat_entries().any(|entry| {
                        entry
                            .ident()
                            .is_some_and(|ident| ident_matches(&ident, name))
                    }) || pattern
                        .pat_bind()
                        .and_then(|bind| bind.ident())
                        .is_some_and(|ident| ident_matches(&ident, name))
                }
            };
            if bound {
                return Some(Binding::Parameter);
            }
        } else {
            // Unqualified inherit resolves in the outer environment, not the
            // recursive environment it introduces a binding into. Attribute
            // names likewise do not use the recursive value environment.
            let outer_environment = ast::Inherit::cast(child.clone())
                .is_some_and(|inherit| inherit.from().is_none())
                || ast::AttrpathValue::cast(child.clone()).is_some_and(|entry| {
                    entry.attrpath().is_some_and(|path| {
                        path.syntax().text_range().contains_range(node.text_range())
                    })
                });
            if !outer_environment {
                let bound = if let Some(let_in) = ast::LetIn::cast(ancestor.clone()) {
                    entries_bind(&let_in, name)
                } else if let Some(legacy) = ast::LegacyLet::cast(ancestor.clone()) {
                    entries_bind(&legacy, name)
                } else if let Some(attrs) = ast::AttrSet::cast(ancestor.clone()) {
                    attrs.rec_token().is_some() && entries_bind(&attrs, name)
                } else {
                    false
                };
                if bound {
                    return Some(Binding::Value);
                }
            }
        }
        child = ancestor;
    }
    None
}

fn entries_bind(scope: &impl HasEntry, name: &str) -> bool {
    scope.entries().any(|entry| match entry {
        ast::Entry::AttrpathValue(entry) => entry
            .attrpath()
            .and_then(|path| path.attrs().next())
            .is_some_and(|attr| attr_matches(&attr, name)),
        ast::Entry::Inherit(inherit) => inherit.attrs().any(|attr| attr_matches(&attr, name)),
    })
}

fn ident_matches(ident: &ast::Ident, name: &str) -> bool {
    ident
        .ident_token()
        .is_some_and(|token| token.text() == name)
}

fn attr_matches(attr: &ast::Attr, name: &str) -> bool {
    match attr {
        ast::Attr::Ident(ident) => ident_matches(ident, name),
        ast::Attr::Str(string) => {
            let mut parts = string.parts();
            match (parts.next(), parts.next()) {
                (Some(InterpolPart::Literal(literal)), None) => {
                    let text = literal.syntax().text();
                    // Escaped or multiline spellings can be ambiguous without
                    // normalization; skip a builtin assumption in that case.
                    text == name || text.contains('\\') || text.contains('\n')
                }
                _ => false,
            }
        }
        ast::Attr::Dynamic(_) => false,
    }
}
