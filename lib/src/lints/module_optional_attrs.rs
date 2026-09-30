use super::module_context::{binary_lib_call, config_module, references_config, supplied_input};
use crate::{Metadata, Report, Rule, Severity};
use macros::lint;
use rnix::{NodeOrToken, SyntaxElement, SyntaxKind, ast::Expr};
use rowan::ast::AstNode as _;

/// ## What it does
/// Advises about a root module configuration built with `lib.optionalAttrs`
/// whose condition directly references the module's supplied `config` input.
///
/// ## Why is this bad?
/// `optionalAttrs` must evaluate its condition to choose the attribute names.
/// This can force `config` while the module system is still constructing it,
/// causing infinite recursion. `mkIf` preserves its content for the module
/// system to process without evaluating the condition at that point.
///
/// Primary evidence:
/// - [`optionalAttrs` implementation](https://github.com/NixOS/nixpkgs/blob/27ccf0e793bfec2ffd9298f53ade75849b581f29/lib/attrsets.nix#L660-L666).
/// - [`mkIf` property processing](https://github.com/NixOS/nixpkgs/blob/4d321931cfde971b3d791a95fb6d22c157ef233a/lib/modules.nix#L876-L883).
/// - [Module recursion and undeclared options](https://discourse.nixos.org/t/optionalattrs-in-module-infinite-recursion-with-config/27876/5).
///
/// This is an advisory, not a proof of recursion or an automatic replacement.
/// `mkIf` still exposes option definitions even when false: the options must
/// exist, and imported declarations cannot be resolved from this syntax alone.
/// Recognition is limited to a pattern lambda supplying `lib` and `config`,
/// an explicit root `config` field with an `options` or `imports` sibling, and
/// a qualified two-argument call with a literal set. Parentheses, let bodies,
/// and root `//` operands are supported. Inner data, aliases of `config`,
/// defaulted inputs, and locally rebound `lib` or `config` are not inferred.
///
/// ## Example
/// ```nix
/// { config, lib, ... }: {
///   options.demo.enable = lib.mkEnableOption "demo";
///   config = lib.optionalAttrs config.demo.enable { demo.message = "on"; };
/// }
/// ```
/// Consider `lib.mkIf` manually after checking the option declarations.
#[lint(
    name = "module_optional_attrs",
    note = "Config-dependent optionalAttrs at a module configuration root",
    code = 29,
    match_with = SyntaxKind::NODE_APPLY
)]
struct ModuleOptionalAttrs;

impl Rule for ModuleOptionalAttrs {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let (condition, member) = binary_lib_call(Expr::cast(node.clone())?, "optionalAttrs")?;
        let module = config_module(node)?;
        if !supplied_input(member.syntax(), &module, "lib")
            || !references_config(&condition, &module)
        {
            return None;
        }
        Some(self.report().severity(Severity::Hint).diagnostic(
            node.text_range(),
            "Root `lib.optionalAttrs` depends on module `config` and may cause infinite recursion; consider `lib.mkIf` manually after checking that its options are declared",
        ))
    }
}
