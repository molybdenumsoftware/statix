use super::module_context::{binary_lib_call, config_module, supplied_input};
use crate::{Metadata, Report, Rule, Severity};
use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{BinOp, BinOpKind},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Advises about a direct `lib.mkIf` operand of `//` at a module's root
/// configuration expression.
///
/// ## Why is this bad?
/// `mkIf` returns a module property record (`_type`, `condition`, `content`),
/// not the conditional plain set that ordinary right-biased `//` expects.
/// `mkMerge` lets the module system combine property records and definitions.
///
/// Primary evidence:
/// - [`mkIf` and `mkMerge` constructors](https://github.com/NixOS/nixpkgs/blob/4d321931cfde971b3d791a95fb6d22c157ef233a/lib/modules.nix#L998-L1011).
/// - [Module property processing](https://github.com/NixOS/nixpkgs/blob/4d321931cfde971b3d791a95fb6d22c157ef233a/lib/modules.nix#L876-L883).
/// - [Advice to avoid combining `mkIf` with `//`](https://discourse.nixos.org/t/lib-modules-mkif-vs-lib-attrsets-optionalattrs-and-other-module-system-basics/42728/6).
///
/// This is a hint with no automatic fix: option merging is not equivalent to
/// right-biased attribute replacement. The appropriate merge and option
/// priorities need a manual review. Recognition is limited to a pattern lambda
/// supplying `lib`, a returned set's explicit `config` field with an `options`
/// or `imports` sibling, and an exact qualified two-argument `mkIf` call with
/// a literal set. Parentheses, let bodies, and root update chains are supported;
/// ordinary data, nested option values, aliases, defaulted inputs, and locally
/// defined libraries are excluded.
///
/// ## Example
/// ```nix
/// { lib, ... }: {
///   options.a = lib.mkEnableOption "a";
///   options.b = lib.mkEnableOption "b";
///   config = (lib.mkIf false { a = true; }) // { b = true; };
/// }
/// ```
/// Consider `lib.mkMerge [ (lib.mkIf false { a = true; }) { b = true; } ]`
/// manually, accounting for the options' merge behavior.
#[lint(
    name = "module_mkif_update",
    note = "Module mkIf combined with an ordinary attribute update",
    code = 28,
    match_with = SyntaxKind::NODE_BIN_OP
)]
struct ModuleMkifUpdate;

impl Rule for ModuleMkifUpdate {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let bin = BinOp::cast(node.clone())?;
        if bin.operator()? != BinOpKind::Update {
            return None;
        }
        let left = binary_lib_call(bin.lhs()?, "mkIf").map(|(_, member)| member);
        let right = binary_lib_call(bin.rhs()?, "mkIf").map(|(_, member)| member);
        if left.is_none() && right.is_none() {
            return None;
        }
        let module = config_module(node)?;
        let has_mkif = left
            .iter()
            .chain(right.iter())
            .any(|member| supplied_input(member.syntax(), &module, "lib"));
        if !has_mkif {
            return None;
        }
        Some(self.report().severity(Severity::Hint).diagnostic(
            node.text_range(),
            "A direct `lib.mkIf` operand of `//` is a module property record, not a plain conditional set; consider `lib.mkMerge` manually because option merging differs from right-biased updates",
        ))
    }
}
