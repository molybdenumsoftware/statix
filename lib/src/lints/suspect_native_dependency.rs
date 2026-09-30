use super::optionals_string::{enclosing_parens, unparen};
use crate::{Metadata, Report, Rule, Severity, utils::has_local_value_binding};
use macros::lint;
use rnix::{
    SyntaxElement, SyntaxKind,
    ast::{Apply, Attr, AttrSet, Entry, Expr, HasEntry, Ident, List},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Advises about `gobject-introspection` in a Python application's host inputs
/// when an explicit native input list includes `wrapGAppsHook` but not
/// `gobject-introspection`.
///
/// ## Why check this?
/// The introspection scanner and setup hook run on the build platform. Their
/// placement matters for cross compilation, but some consumers also link the
/// host `libgirepository` library. A package name alone cannot justify moving
/// or removing a dependency.
///
/// Primary evidence:
/// - [Scanner/hook placement changes, including hushboard's Python builder](https://github.com/NixOS/nixpkgs/pull/239191).
/// - [Review warning that cjs probably links the library](https://github.com/NixOS/nixpkgs/pull/239191#discussion_r1238464808).
/// - [Review identifying a host libgirepository-only consumer](https://github.com/NixOS/nixpkgs/pull/239191#discussion_r1238469231).
///
/// This is a contextual hint, not proof that the package is wrong. It supplies
/// no automatic fix: retain host libraries when linked, and check whether the
/// native scanner/setup hook is required. Recognition is deliberately limited
/// to unqualified conventional `buildPythonApplication` calls with a direct
/// non-recursive literal argument set and explicit literal dependency lists.
/// Each dependency must be a bare conventional package identifier. Local
/// definitions, selected outputs, conditional/dynamic dependencies, inherited
/// dependency fields, recursive arguments, and custom builders are excluded.
///
/// ## Example
/// ```nix
/// { buildPythonApplication, wrapGAppsHook, gobject-introspection, gtk3, ... }:
/// buildPythonApplication {
///   nativeBuildInputs = [ wrapGAppsHook ];
///   buildInputs = [ gobject-introspection gtk3 ];
/// }
/// ```
/// Check native scanner/setup hook placement for cross compilation manually;
/// retain the host library dependency if this application links it.
#[lint(
    name = "suspect_native_dependency",
    note = "Check introspection scanner and setup hook placement for cross compilation",
    code = 37,
    match_with = SyntaxKind::NODE_APPLY
)]
struct SuspectNativeDependency;

impl Rule for SuspectNativeDependency {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let apply = Apply::cast(node.as_node()?.clone())?;
        let Expr::Ident(builder) = unparen(apply.lambda()?)? else {
            return None;
        };
        if builder.ident_token()?.text() != "buildPythonApplication"
            || has_local_value_binding(builder.syntax(), "buildPythonApplication")
        {
            return None;
        }
        // Do not interpret the first argument of an unknown multi-argument call
        // as a conventional Python application definition.
        let operand = enclosing_parens(apply.syntax().clone());
        if operand.parent().and_then(Apply::cast).is_some_and(|outer| {
            outer
                .lambda()
                .is_some_and(|lambda| lambda.syntax() == &operand)
        }) {
            return None;
        }
        let Expr::AttrSet(attrs) = unparen(apply.argument()?)? else {
            return None;
        };
        if attrs.rec_token().is_some() {
            return None;
        }
        let (native, host) = dependency_lists(&attrs)?;
        let native = dependencies(&native)?;
        if !native.wrap_hook || native.introspection.is_some() {
            return None;
        }
        let host = dependencies(&host)?.introspection?;
        Some(self.report().severity(Severity::Hint).diagnostic(
            host.syntax().text_range(),
            "With `wrapGAppsHook`, check native `gobject-introspection` scanner/setup hook placement for cross compilation; retain host libraries when linked",
        ))
    }
}

fn dependency_lists(attrs: &AttrSet) -> Option<(List, List)> {
    let mut native = None;
    let mut host = None;
    for entry in attrs.entries() {
        let entry = match entry {
            Entry::AttrpathValue(entry) => entry,
            Entry::Inherit(inherit) => {
                for attr in inherit.attrs() {
                    let Attr::Ident(ident) = attr else {
                        return None;
                    };
                    if matches!(
                        ident.ident_token()?.text(),
                        "nativeBuildInputs" | "buildInputs"
                    ) {
                        return None;
                    }
                }
                continue;
            }
        };
        let mut path = entry.attrpath()?.attrs();
        let Attr::Ident(key) = path.next()? else {
            // A computed key may introduce either dependency field.
            return None;
        };
        let target = match key.ident_token()?.text() {
            "nativeBuildInputs" => &mut native,
            "buildInputs" => &mut host,
            _ => continue,
        };
        if target.is_some() || path.next().is_some() {
            return None;
        }
        let Expr::List(list) = unparen(entry.value()?)? else {
            return None;
        };
        *target = Some(list);
    }
    Some((native?, host?))
}

struct Dependencies {
    wrap_hook: bool,
    introspection: Option<Ident>,
}

fn dependencies(list: &List) -> Option<Dependencies> {
    let mut result = Dependencies {
        wrap_hook: false,
        introspection: None,
    };
    for item in list.items() {
        let Expr::Ident(ident) = unparen(item)? else {
            return None;
        };
        let token = ident.ident_token()?;
        if has_local_value_binding(ident.syntax(), token.text()) {
            return None;
        }
        match token.text() {
            "wrapGAppsHook" => result.wrap_hook = true,
            "gobject-introspection" => result.introspection = Some(ident),
            _ => {}
        }
    }
    Some(result)
}
