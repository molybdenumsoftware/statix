use crate::{Metadata, Report, Rule};

use macros::lint;
use rnix::{NodeOrToken, SyntaxElement, SyntaxKind, TextRange, TextSize};

/// ## What it does
/// Checks for paths into non-Nix system locations like `/usr/local/`,
/// `/opt/homebrew/`, `/usr/bin/` (except `/usr/bin/env`) and `/bin/bash`.
///
/// ## Why is this bad?
/// These depend on what happens to be installed on the host, so the result
/// differs between machines and doesn't exist on NixOS.
///
/// ## Example
///
/// ```nix
/// "/usr/local/bin/jq . data.json"
/// ```
///
/// Use a package from nixpkgs instead:
///
/// ```nix
/// "${pkgs.jq}/bin/jq . data.json"
/// ```
#[lint(
    name = "impure_host_path",
    note = "Found path outside the Nix store",
    code = 27,
    match_with = [SyntaxKind::TOKEN_STRING_CONTENT, SyntaxKind::TOKEN_PATH_ABS]
)]
struct ImpureHostPath;

const HOST_PATHS: &[&str] = &["/usr/local/", "/opt/homebrew/", "/usr/bin/", "/bin/bash"];

const HELP: &str = "If it's a program, use it from nixpkgs: `${pkgs.jq}/bin/jq` or `${lib.getExe pkgs.jq}` \
(in devenv, adding the package to `packages` and calling it by name also works). \
If it's where files are read or written, use a location the project owns, like `$DEVENV_ROOT`, \
`$DEVENV_STATE` or a path passed in as an argument. \
Don't just swap in another host path such as `/tmp`, and keep what the code does the same.";

fn is_path_char(c: u8) -> bool {
    // `@out@/usr/bin`, `{bash}/bin/bash` are prefixed paths too
    c.is_ascii_alphanumeric() || b"/._-+@}".contains(&c)
}

/// Commands that rewrite host paths into store paths: mentioning the host
/// path there is the fix, not the problem.
const REWRITES: &[&str] = &["--replace", "substitute", "sed ", "s|", "s!", "s#", "s,"];

fn is_rewrite(line_before: &str) -> bool {
    REWRITES.iter().any(|r| line_before.contains(r))
}

impl Rule for ImpureHostPath {
    fn validate(&self, node: &SyntaxElement) -> Option<Report> {
        let NodeOrToken::Token(token) = node else {
            return None;
        };
        let text = token.text();
        let start = token.text_range().start();
        // `${pkgs.bash}/bin/bash`: the token continues an interpolated path
        let after_interpolation = token
            .prev_sibling_or_token()
            .is_some_and(|prev| prev.kind() == SyntaxKind::NODE_INTERPOL);

        let mut hits: Vec<(usize, &str)> = HOST_PATHS
            .iter()
            .flat_map(|host| text.match_indices(host))
            // only where a path starts, `/usr/local/bin/bash` is one hit
            .filter(|&(i, _)| {
                if i == 0 {
                    !after_interpolation
                } else {
                    !is_path_char(text.as_bytes()[i - 1])
                }
            })
            .filter(|&(i, host)| host != "/usr/bin/" || !text[i..].starts_with("/usr/bin/env"))
            .filter(|&(i, _)| {
                let line_start = text[..i].rfind('\n').map_or(0, |n| n + 1);
                !is_rewrite(&text[line_start..i])
            })
            .collect();
        hits.sort_unstable();

        let mut report = self.report();
        for (i, host) in hits {
            let len = text[i..]
                .find(|c: char| c.is_whitespace() || "\"'`;:)".contains(c))
                .unwrap_or(text.len() - i);
            let at = TextRange::at(
                start + TextSize::try_from(i).ok()?,
                TextSize::try_from(len).ok()?,
            );
            report = report.diagnostic_with_help(
                at,
                format!("`{host}` depends on the host system, use a package from nixpkgs instead"),
                HELP.to_string(),
            );
        }
        (!report.diagnostics.is_empty()).then_some(report)
    }
}
