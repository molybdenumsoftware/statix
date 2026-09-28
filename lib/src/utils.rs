use rnix::{SyntaxKind, SyntaxNode, TextRange};

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

/// Static name of an attribute (`foo`, `"foo"`), `None` for dynamic keys.
pub fn attr_name(attr: &rnix::ast::Attr) -> Option<String> {
    use rnix::ast::{Attr, InterpolPart};
    use rowan::ast::AstNode as _;
    match attr {
        Attr::Ident(ident) => Some(ident.syntax().text().to_string()),
        Attr::Str(s) => s
            .normalized_parts()
            .into_iter()
            .map(|part| match part {
                InterpolPart::Literal(lit) => Some(lit),
                InterpolPart::Interpolation(_) => None,
            })
            .collect(),
        Attr::Dynamic(_) => None,
    }
}

/// Keys of the attribute sets enclosing `node`, outermost first, e.g. for `x` in
/// `{ a = { b.c = x; }; }` this is `["a", "b", "c"]`. Dynamic keys
/// (`${name}`) are `*`. Stops at `let` bindings: for `x` in
/// `let y = { a = x; }; in ...` this is `["a"]`.
pub fn enclosing_attrpath(node: &SyntaxNode) -> Option<Vec<String>> {
    use rnix::ast::AttrpathValue;
    use rowan::ast::AstNode as _;
    let mut keys = Vec::new();
    for ancestor in node.ancestors() {
        let Some(apv) = AttrpathValue::cast(ancestor) else {
            continue;
        };
        if apv.syntax().parent()?.kind() != SyntaxKind::NODE_ATTR_SET {
            break;
        }
        let names = apv
            .attrpath()?
            .attrs()
            .map(|a| attr_name(&a).unwrap_or_else(|| "*".to_string()));
        keys.splice(0..0, names);
    }
    Some(keys)
}

/// The `package = ...;` next to `exec` (`scripts.x = { exec; package; }` or
/// `scripts.x.exec` + `scripts.x.package`), as written.
pub fn sibling_package(exec: &rnix::ast::AttrpathValue) -> Option<String> {
    use rnix::ast::HasEntry as _;
    use rowan::ast::AstNode as _;
    let keys = |apv: &rnix::ast::AttrpathValue| -> Vec<String> {
        apv.attrpath()
            .map(|p| p.attrs().filter_map(|a| attr_name(&a)).collect())
            .unwrap_or_default()
    };
    let set = rnix::ast::AttrSet::cast(exec.syntax().parent()?)?;
    let mut package_keys = keys(exec);
    package_keys.pop();
    package_keys.push("package".into());
    set.attrpath_values()
        .find(|sibling| keys(sibling) == package_keys)
        .and_then(|sibling| sibling.value())
        .map(|value| value.syntax().to_string())
}
