use crate::{Metadata, Report, Rule, utils};

use macros::lint;
use rnix::{
    NodeOrToken, SyntaxElement, SyntaxKind,
    ast::{AttrSet, AttrpathValue, Expr, HasEntry as _, InterpolPart},
};
use rowan::ast::AstNode as _;

/// ## What it does
/// Checks for a shebang at the start of devenv's `scripts.<name>.exec`,
/// `tasks.<name>.exec` or `processes.<name>.exec`.
///
/// ## Why is this bad?
/// devenv writes `exec` to a file and runs it with the interpreter from
/// `package` (bash by default), so the shebang is just a comment. A
/// `#!/usr/bin/env python3` script is silently run by bash.
///
/// ## Example
///
/// ```nix
/// {
///   scripts.hello.exec = ''
///     #!/usr/bin/env python3
///     print("hello")
///   '';
/// }
/// ```
///
/// Set the interpreter with `package` instead:
///
/// ```nix
/// {
///   scripts.hello = {
///     package = pkgs.python3;
///     exec = ''
///       print("hello")
///     '';
///   };
/// }
/// ```
#[lint(
    name = "devenv_exec_shebang",
    note = "Shebang in devenv `exec` is ignored",
    code = 24,
    match_with = SyntaxKind::NODE_ATTRPATH_VALUE
)]
struct DevenvExecShebang;

const SHELL_PACKAGES: &[&str] = &["bash", "bashInteractive"];

impl Rule for DevenvExecShebang {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Node(node) = node else {
            return None;
        };
        let apv = AttrpathValue::cast(node.clone())?;
        let path = utils::enclosing_attrpath(node)?;
        let path = path.strip_prefix(&["config".to_string()]).unwrap_or(&path);
        let [kind, _, exec] = path else {
            return None;
        };
        if exec != "exec" || !["scripts", "tasks", "processes"].contains(&kind.as_str()) {
            return None;
        }
        let Some(Expr::Str(value)) = apv.value() else {
            return None;
        };

        // Text before the first interpolation is enough to find a shebang.
        let first_literal = match value.normalized_parts().into_iter().next()? {
            InterpolPart::Literal(lit) => lit,
            InterpolPart::Interpolation(_) => return None,
        };
        let first_line = first_literal.lines().find(|l| !l.trim().is_empty())?;
        if !first_line.trim_start().starts_with("#!") {
            return None;
        }
        if has_non_shell_package(&apv) {
            return None;
        }

        let message = format!(
            "devenv runs `{kind}.<name>.exec` with bash, this shebang is ignored; set `package` to choose the interpreter"
        );
        Some(
            self.report()
                .diagnostic(value.syntax().text_range(), message),
        )
    }
}

/// Is there a sibling `package = pkgs.<not bash>;` next to this `exec`?
fn has_non_shell_package(exec: &AttrpathValue) -> bool {
    let Some(exec_keys) = exec.attrpath().map(|p| p.attrs().filter_map(|a| utils::attr_name(&a)).collect::<Vec<_>>()) else {
        return false;
    };
    let Some(set) = exec.syntax().parent().and_then(AttrSet::cast) else {
        return false;
    };
    let mut package_keys = exec_keys;
    package_keys.pop();
    package_keys.push("package".into());

    set.attrpath_values().any(|sibling| {
        let keys: Vec<_> = sibling
            .attrpath()
            .map(|p| p.attrs().filter_map(|a| utils::attr_name(&a)).collect())
            .unwrap_or_default();
        let Some(value) = sibling.value() else {
            return false;
        };
        let value = value.syntax().to_string();
        let last = value.trim().rsplit('.').next().unwrap_or_default();
        keys == package_keys && !SHELL_PACKAGES.contains(&last)
    })
}
