use crate::{Metadata, Report, Rule, Suggestion, make, utils};

use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{AttrpathValue, Expr, HasEntry as _},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Checks for the deprecated devenv option `pre-commit.hooks`.
///
/// ## Why is this bad?
/// devenv renamed the `pre-commit` options to `git-hooks`; the old name is
/// only kept as an alias. Only top-level `pre-commit.hooks` (devenv) is
/// matched, not `pre-commit.settings.hooks` of the git-hooks.nix flake-parts
/// module.
///
/// ## Example
///
/// ```nix
/// {
///   pre-commit.hooks.shellcheck.enable = true;
/// }
/// ```
///
/// Use `git-hooks` instead:
///
/// ```nix
/// {
///   git-hooks.hooks.shellcheck.enable = true;
/// }
/// ```
#[lint(
    name = "devenv_pre_commit",
    note = "Found deprecated devenv option `pre-commit`",
    code = 25,
    match_with = SyntaxKind::NODE_ATTRPATH_VALUE
)]
struct DevenvPreCommit;

impl Rule for DevenvPreCommit {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let apv = AttrpathValue::cast(node.clone())?;
        let path = utils::enclosing_attrpath(node)?;
        let own_len = apv.attrpath()?.attrs().count();
        let offset = usize::from(path.first().is_some_and(|k| k == "config"));

        // `pre-commit` must be a key of this binding, at the module's top level.
        let index = path.iter().position(|k| k == "pre-commit")?;
        if index != offset || index < path.len() - own_len {
            return None;
        }
        let hooks_follows = match path.get(index + 1) {
            Some(next) => next == "hooks",
            None => match apv.value()? {
                Expr::AttrSet(set) => set.attrpath_values().any(|entry| {
                    entry
                        .attrpath()
                        .and_then(|p| p.attrs().next())
                        .and_then(|a| utils::attr_name(&a))
                        .is_some_and(|k| k == "hooks")
                }),
                _ => false,
            },
        };
        if !hooks_follows {
            return None;
        }

        let attr = apv
            .attrpath()?
            .attrs()
            .nth(index - (path.len() - own_len))?;
        let at = attr.syntax().text_range();
        let replacement = make::ident("git-hooks");
        Some(self.report().suggest(
            at,
            "`pre-commit` is deprecated in devenv, use `git-hooks`",
            Suggestion::with_replacement(at, replacement.syntax().clone()),
        ))
    }
}
