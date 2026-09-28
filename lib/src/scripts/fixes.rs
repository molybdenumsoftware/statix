//! Fixes for shellcheck findings that ship without one, and hints for the
//! ones that need a human (or an agent) to decide.

use serde::{Deserialize, Serialize};

/// One edit in 1-based shellcheck coordinates of the rendered script
/// (`end_*` exclusive), same shape as shellcheck's `json1` replacements.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub replacement: String,
}

/// Findings that only exist because `${...}` is rendered as a placeholder
/// word: the real value is a list, an absolute path, a brace expansion...
pub const PLACEHOLDER_ARTIFACTS: &[u32] = &[
    1083, // `{${list}}` looks like a literal brace
    2043, // `for x in ${list}` looks like a single word
    2050, 2078, 2157, 2194, // constant expressions
    2123, // `PATH=${lib.makeBinPath ...}` is deliberate
    2239, // `#!${bash}/bin/bash` looks relative
];

/// Pointers to an earlier parse error, not findings of their own.
pub const META: &[u32] = &[1072, 1073];

/// Variables and functions that the surrounding environment (stdenv, the
/// outer script, other activation snippets) provides.
pub const HOOK_NOISE: &[u32] = &[2034, 2154];

fn is_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Mechanical fixes for codes where shellcheck gives none.
pub fn builtin(
    code: u32,
    text: &str,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
) -> Option<Vec<Change>> {
    let src_line = text.split('\n').nth(line.checked_sub(1)?)?;
    let chars: Vec<char> = src_line.chars().collect();
    let range: String = if end_line == line {
        chars.get(column - 1..end_column - 1)?.iter().collect()
    } else {
        chars.get(column - 1..)?.iter().collect()
    };
    let change = |column, end_column, replacement: String| Change {
        line,
        column,
        end_line: line,
        end_column,
        replacement,
    };

    match code {
        // `read x` -> `read -r x`
        2162 if range.starts_with("read") => {
            Some(vec![change(column + 4, column + 4, " -r".into())])
        }

        // `rm -rf "$DIR"/*` -> `rm -rf "${DIR:?}"/*`
        2115 => {
            let quoted = range.starts_with('"');
            let inner = range.trim_start_matches('"').strip_prefix('$')?;
            let (name, rest) = if let Some(braced) = inner.strip_prefix('{') {
                let end = braced.find('}')?;
                (&braced[..end], &braced[end + 1..])
            } else {
                let end = inner
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(inner.len());
                (&inner[..end], &inner[end..])
            };
            if !is_name(name) {
                return None;
            }
            let rest = if quoted {
                rest.strip_prefix('"')?
            } else {
                rest
            };
            let replaced_len = range.chars().count() - rest.chars().count();
            Some(vec![change(
                column,
                column + replaced_len,
                format!("\"${{{name}:?}}\""),
            )])
        }

        // `for f in $(ls *.txt)` -> `for f in *.txt`
        2045 => {
            let args = range.strip_prefix("$(ls ")?.strip_suffix(')')?;
            let simple = !args.is_empty()
                && !args.split_whitespace().any(|w| w.starts_with('-'))
                && !args.contains(['|', ';', '&', '$', '`', '(', ')', '<', '>']);
            simple.then(|| vec![change(column, end_column, args.trim().to_string())])
        }

        // `export X=$(cmd)` -> `export X` + `X=$(cmd)` so failures aren't masked
        2155 => {
            let indent_len = src_line.len() - src_line.trim_start().len();
            let indent = &src_line[..indent_len];
            let rest = &src_line[indent_len..];
            let (keyword, assignment) = rest.split_once(char::is_whitespace)?;
            if !["export", "local", "declare"].contains(&keyword) {
                return None;
            }
            let (name, value) = assignment.trim_start().split_once('=')?;
            let value = value.trim_end();
            let substitution = value.strip_prefix('"').unwrap_or(value);
            let single = substitution.starts_with("$(")
                && (value.ends_with(')') || value.ends_with(")\""))
                && !value.contains([';', '&', '|'])
                && !value.contains(") ");
            if !is_name(name) || !single {
                return None;
            }
            Some(vec![Change {
                line,
                column: indent_len + 1,
                end_line: line,
                end_column: src_line.chars().count() + 1,
                replacement: format!("{keyword} {name}\n{indent}{name}={value}"),
            }])
        }
        _ => None,
    }
}

/// How to fix a finding by hand, for codes that commonly show up in Nix.
pub fn hint(code: u32) -> Option<&'static str> {
    Some(match code {
        2046 => {
            "Quote the command substitution (\"$(cmd)\") if it yields one value; if it must split into several words, read it into an array (mapfile -t arr < <(cmd)) and use \"${arr[@]}\"."
        }
        2034 => {
            "The variable is never used in this script. Remove it, or export it if another process reads it."
        }
        2317 => {
            "This code is unreachable (usually after exit/exec or a function that always exits). Remove it or fix the control flow."
        }
        2154 => {
            "The variable is never assigned in this script. If devenv/Nix provides it (env.*, $out, $DEVENV_ROOT), use \"${VAR:?}\" or add `# shellcheck disable=SC2154`; otherwise fix the name."
        }
        2206 | 2207 => {
            "Word-splitting into an array: use mapfile -t arr < <(cmd) or read -ra arr <<< \"$var\"."
        }
        2145 | 2068 => {
            "Use \"$@\" / \"${arr[@]}\" as separate quoted words instead of concatenating them into a string."
        }
        2091 => "$(cmd) runs the command's output as a command. Drop the $( ) to just run cmd.",
        2209 => {
            "To store a command's output use var=$(cmd); `var=cmd` only assigns the literal word."
        }
        2016 => {
            "Expressions don't expand in single quotes. Use double quotes if expansion is intended; otherwise ignore with `# shellcheck disable=SC2016`."
        }
        2164 => "Add `|| exit` (or `|| return` in a function) so the script stops if cd fails.",
        2155 => {
            "Declare and assign separately: `export X` then `X=$(cmd)`, so cmd's exit status isn't masked."
        }
        2181 => "Check the command directly: `if cmd; then` instead of `cmd; if [ $? -eq 0 ]`.",
        2044 | 2045 => {
            "Iterate with a glob (for f in dir/*) or find -print0 | while IFS= read -r -d '' f."
        }
        1010 => {
            "A keyword like `done`/`fi` appears where a word is expected. If it's meant as text (`echo done`), quote it: `echo 'done'`. Only if it should end a loop/if, put `;` or a newline before it. Change nothing else."
        }
        _ => return None,
    })
}
