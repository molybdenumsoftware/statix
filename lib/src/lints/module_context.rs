//! Conservative syntax-only module context shared by the module advisories.
//!
//! Nixpkgs recognizes explicit `config`/`options` and shorthand module syntax:
//! <https://github.com/NixOS/nixpkgs/blob/4d321931cfde971b3d791a95fb6d22c157ef233a/lib/modules.nix#L454-L492>
//! Here we deliberately recognize only a pattern lambda supplying `lib`, whose
//! returned set has an explicit `config = ...` plus `options` or `imports`.
//! This is evidence of module intent, not proof that evalModules consumes it.
//! Shorthand modules, aliases, computed returns, and inner option/data values
//! are excluded. Only parentheses, let bodies, and `//` operands preserve the
//! root configuration context; arbitrary applications do not.

use super::optionals_string::{enclosing_parens, lib_member, unparen};
use crate::utils::has_local_value_binding;
use rnix::{
    SyntaxKind, SyntaxNode,
    ast::{
        Attr, AttrSet, AttrpathValue, BinOp, BinOpKind, Expr, HasEntry, Ident, Lambda, LetIn, Param,
    },
};
use rowan::ast::AstNode as _;

/// Find the module lambda only for expressions forming its root configuration.
pub(super) fn config_module(node: &SyntaxNode) -> Option<Lambda> {
    let mut current = enclosing_parens(node.clone());
    loop {
        let parent = current.parent()?;
        if let Some(let_in) = LetIn::cast(parent.clone()) {
            if let_in.body()?.syntax() != &current {
                return None;
            }
            current = enclosing_parens(parent);
        } else if let Some(bin) = BinOp::cast(parent.clone()) {
            if bin.operator()? != BinOpKind::Update
                || (bin.lhs()?.syntax() != &current && bin.rhs()?.syntax() != &current)
            {
                return None;
            }
            current = enclosing_parens(parent);
        } else {
            let entry = AttrpathValue::cast(parent)?;
            if entry.value()?.syntax() != &current || !single_key(&entry, "config") {
                return None;
            }
            let attrs = AttrSet::cast(entry.syntax().parent()?)?;
            if !attrs.attrpath_values().any(|entry| {
                entry.attrpath().is_some_and(|path| {
                    path.attrs().next().is_some_and(|attr| {
                        matches!(attr, Attr::Ident(ident) if ident_matches(&ident, "options") || ident_matches(&ident, "imports"))
                    })
                })
            }) {
                return None;
            }
            let module = returning_lambda(attrs.syntax())?;
            return has_supplied_input(&module, "lib").then_some(module);
        }
    }
}

fn returning_lambda(node: &SyntaxNode) -> Option<Lambda> {
    let mut current = enclosing_parens(node.clone());
    loop {
        let parent = current.parent()?;
        if let Some(let_in) = LetIn::cast(parent.clone()) {
            if let_in.body()?.syntax() != &current {
                return None;
            }
            current = enclosing_parens(parent);
        } else {
            let lambda = Lambda::cast(parent)?;
            return (lambda.body()?.syntax() == &current).then_some(lambda);
        }
    }
}

fn single_key(entry: &AttrpathValue, name: &str) -> bool {
    entry.attrpath().is_some_and(|path| {
        let mut attrs = path.attrs();
        matches!(attrs.next(), Some(Attr::Ident(ident)) if ident_matches(&ident, name))
            && attrs.next().is_none()
    })
}

fn has_supplied_input(lambda: &Lambda, name: &str) -> bool {
    let Some(Param::Pattern(pattern)) = lambda.param() else {
        return false;
    };
    pattern.pat_entries().any(|entry| {
        entry.question_token().is_none()
            && entry
                .ident()
                .is_some_and(|ident| ident_matches(&ident, name))
    })
}

/// Keep the exact conventional module input, not an inner parameter or value.
pub(super) fn supplied_input(node: &SyntaxNode, module: &Lambda, name: &str) -> bool {
    if !has_supplied_input(module, name) || has_local_value_binding(node, name) {
        return false;
    }
    for ancestor in node.ancestors().skip(1) {
        let Some(lambda) = Lambda::cast(ancestor) else {
            continue;
        };
        let binds = match lambda.param() {
            Some(Param::IdentParam(param)) => param
                .ident()
                .is_some_and(|ident| ident_matches(&ident, name)),
            Some(Param::Pattern(pattern)) => {
                pattern.pat_entries().any(|entry| {
                    entry
                        .ident()
                        .is_some_and(|ident| ident_matches(&ident, name))
                }) || pattern
                    .pat_bind()
                    .and_then(|bind| bind.ident())
                    .is_some_and(|ident| ident_matches(&ident, name))
            }
            None => return false,
        };
        if binds {
            return lambda.syntax() == module.syntax();
        }
    }
    false
}

/// Recognize only the full two-argument qualified helper with a literal set.
/// The caller's root-context check excludes partially or additionally applied
/// calls. A local value named `lib` is also rejected by `lib_member`.
pub(super) fn binary_lib_call(expr: Expr, member: &str) -> Option<(Expr, Ident)> {
    let Expr::Apply(second) = unparen(expr)? else {
        return None;
    };
    if !matches!(unparen(second.argument()?)?, Expr::AttrSet(_)) {
        return None;
    }
    let Expr::Apply(first) = unparen(second.lambda()?)? else {
        return None;
    };
    let member = lib_member(first.lambda()?, member)?;
    Some((first.argument()?, member))
}

/// Look for expression references to the supplied config, not attribute names.
/// Alias chains and shorthand `inherit config` are intentionally not followed.
pub(super) fn references_config(condition: &Expr, module: &Lambda) -> bool {
    condition.syntax().descendants().any(|node| {
        let Some(ident) = Ident::cast(node) else {
            return false;
        };
        if !ident_matches(&ident, "config") {
            return false;
        }
        let Some(parent) = ident.syntax().parent() else {
            return false;
        };
        if matches!(
            parent.kind(),
            SyntaxKind::NODE_ATTRPATH
                | SyntaxKind::NODE_IDENT_PARAM
                | SyntaxKind::NODE_PAT_ENTRY
                | SyntaxKind::NODE_PAT_BIND
                | SyntaxKind::NODE_INHERIT
        ) {
            return false;
        }
        supplied_input(ident.syntax(), module, "config")
    })
}

fn ident_matches(ident: &Ident, name: &str) -> bool {
    ident
        .ident_token()
        .is_some_and(|token| token.text() == name)
}
