use lib::{LINTS, Report, Severity};

const RULES: &[&str] = &["shell_double_escaping", "shell_unquoted_substitution"];

fn reports(source: &str) -> Vec<Report> {
    let parsed = rnix::Root::parse(source);
    assert!(
        parsed.errors().is_empty(),
        "invalid fixture: {:?}: {source}",
        parsed.errors()
    );
    parsed
        .syntax()
        .descendants()
        .flat_map(|node| {
            let kind = node.kind();
            LINTS
                .iter()
                .filter(move |lint| RULES.contains(&lint.name()) && lint.match_with(&kind))
                .filter_map(move |lint| lint.validate(&node.clone().into()))
        })
        .collect()
}

fn assert_hint(source: &str, name: &str, code: u32) {
    let reports = reports(source);
    assert_eq!(
        reports
            .iter()
            .map(|report| (report.name, report.code))
            .collect::<Vec<_>>(),
        vec![(name, code)],
        "{source}"
    );
    assert!(matches!(reports[0].severity, Severity::Hint), "{source}");
    assert!(
        reports[0]
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.suggestion.is_none()),
        "{source}"
    );
    let mut unchanged = source.to_owned();
    reports[0].apply(&mut unchanged);
    assert_eq!(unchanged, source, "advice must never rewrite shell quoting");
}

#[test]
fn whole_word_outer_single_quotes_are_advisory_only() {
    for source in [
        r"{ lib, filePath }: { script = ''cat '${lib.escapeShellArg filePath}' ''; }",
        r#"{ lib, filePath }: { preBuild = "cat '${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath }: { shellHook = "cat '${(lib.escapeShellArg) (filePath)}'"; }"#,
        r#"{ lib, filePath }: { installPhase = "cat '${lib.escapeShellArgs [ filePath ]}'"; }"#,
        r#"{ lib, x, y }: { script = "printf '%s' '${lib.escapeShellArgs [ x y ]}'"; }"#,
        r#"{ pkgs, lib, filePath }: pkgs.writeShellScript "run" "cat '${lib.escapeShellArg filePath}'""#,
        r#"{ pkgs, lib, filePath }: pkgs.writeShellScriptBin "run" "cat '${lib.escapeShellArg filePath}'""#,
        r#"{ pkgs, lib, filePath }: pkgs.writeShellApplication { name = "run"; text = "cat '${lib.escapeShellArg filePath}'"; }"#,
    ] {
        assert_hint(source, "shell_double_escaping", 33);
    }
}

#[test]
fn dirname_output_has_a_separate_conditional_quoting_hazard() {
    for source in [
        r"{ lib, filePath }: { script = ''mkdir -p $(dirname ${lib.escapeShellArg filePath})''; }",
        r#"{ lib, filePath }: { script = "mkdir -p $(dirname -- ${lib.escapeShellArg filePath})"; }"#,
        r#"{ lib, filePath }: { script = "mkdir -p $(dirname ${lib.escapeShellArg filePath} )"; }"#,
        r#"{ script = ''mkdir -p $(dirname "$file")''; }"#,
        r"{ script = ''mkdir -p $(dirname '/path with spaces/file')''; }",
        r"{ script = ''mkdir -p $(dirname $file)''; }",
        r#"{ lib, filePath }: { script = "echo ${lib.escapeShellArg filePath}; mkdir -p $(dirname ${lib.escapeShellArg filePath})"; }"#,
    ] {
        assert_hint(source, "shell_unquoted_substitution", 34);
    }
}

#[test]
fn non_shell_consumers_and_unknown_provenance_are_not_checked() {
    for source in [
        r#"{ lib, filePath }: { documentation = "cat '${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath }: { sql = "SELECT '${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath }: { description = "mkdir -p $(dirname ${lib.escapeShellArg filePath})"; }"#,
        r#"{ lib, filePath }: "cat '${lib.escapeShellArg filePath}'""#,
        r#"{ lib, filePath }: { text = "cat '${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath }: other.writeShellScript "run" "cat '${lib.escapeShellArg filePath}'""#,
        r#"{ lib, filePath }: let pkgs = custom; in pkgs.writeShellScript "run" "cat '${lib.escapeShellArg filePath}'""#,
        r#"{ filePath }: let lib = custom; in { script = "cat '${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath }: let lib = custom; in { script = "cat '${lib.escapeShellArg filePath}'"; }"#,
        r#"{ filePath }: rec { lib = custom; script = "cat '${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib ? custom, filePath }: { script = "cat '${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib ? custom, filePath }: { script = "mkdir -p $(dirname ${lib.escapeShellArg filePath})"; }"#,
        r#"{ pkgs ? custom, lib, filePath }: pkgs.writeShellScript "run" "cat '${lib.escapeShellArg filePath}'""#,
        r#"{ lib, filePath }: { script = "cat '${escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath }: { script = "cat '${lib.strings.escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath }: { script = "cat '${(lib.escapeShellArg or fallback) filePath}'"; }"#,
        r#"{ lib, filePath }: { script = "cat '${lib.escapeShellArg filePath extra}'"; }"#,
        r#"{ lib, xs }: { script = "cat '${lib.escapeShellArgs xs}'"; }"#,
        r#"{ lib }: { script = "cat '${lib.escapeShellArgs []}'"; }"#,
        // The cited PR's actual inherited-helper/callable shape is not covered
        // by the baseline's qualified helper and direct-consumer recognition.
        r"{ lib }: let inherit (lib) escapeShellArg; impureConfigMerger = filePath: ''cat '${escapeShellArg filePath}' ''; in { script = impureConfigMerger path; }",
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
}

#[test]
fn adjacent_quote_segments_and_composed_words_are_not_guessed() {
    for source in [
        r#"{ lib, filePath }: { script = "cat ${lib.escapeShellArg filePath}"; }"#,
        r#"{ lib, filePath }: { script = ''cat "${lib.escapeShellArg filePath}"''; }"#,
        r#"{ lib, filePath }: { script = "cat pre'${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath }: { script = "cat '${lib.escapeShellArg filePath}'post"; }"#,
        r#"{ lib, filePath }: { script = "cat '''${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath }: { script = "cat '${lib.escapeShellArg filePath}'''"; }"#,
        r#"{ lib, filePath }: { script = "cat \\'${lib.escapeShellArg filePath}\\'"; }"#,
        r#"{ lib, filePath, tail }: { script = "cat '${lib.escapeShellArg filePath}'${tail}"; }"#,
        r#"{ lib, filePath }: { script = "'${lib.escapeShellArg filePath}'"; }"#,
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
}

#[test]
fn quote_comments_and_nix_escapes_preserve_shell_context() {
    for source in [
        r#"{ lib, filePath }: { script = ''mkdir -p "$(dirname ${lib.escapeShellArg filePath})"''; }"#,
        r#"{ lib, filePath }: { script = "echo '$(dirname ${lib.escapeShellArg filePath})'"; }"#,
        r##"{ lib, filePath }: { script = "# cat '${lib.escapeShellArg filePath}'\necho done"; }"##,
        r##"{ lib, filePath }: { script = "# mkdir -p $(dirname ${lib.escapeShellArg filePath})\necho done"; }"##,
        r#"{ lib, filePath }: { script = ''echo "cat '${lib.escapeShellArg filePath}'"''; }"#,
        r#"{ script = ''echo "\$(dirname "$file")"''; }"#,
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
    // Nix decoding turns \n into a shell newline; a comment does not swallow
    // the subsequent genuine command. Backslash-apostrophe stays unquoted.
    assert_hint(
        r##"{ lib, filePath }: { script = "# docs\ncat '${lib.escapeShellArg filePath}'"; }"##,
        "shell_double_escaping",
        33,
    );
    assert_hint(
        r#"{ lib, filePath }: { script = "echo it\\'s; cat '${lib.escapeShellArg filePath}'"; }"#,
        "shell_double_escaping",
        33,
    );
}

#[test]
fn unsupported_shell_modes_and_unknown_prior_holes_are_skipped() {
    for source in [
        r#"{ lib, filePath }: { script = "cat <<EOF\n'${lib.escapeShellArg filePath}'\nEOF"; }"#,
        r#"{ lib, filePath }: { script = "cat '${lib.escapeShellArg filePath}'; cat <<EOF\ntext\nEOF"; }"#,
        r"{ lib, filePath }: { script = ''echo `cat '${lib.escapeShellArg filePath}'`''; }",
        r"{ lib, filePath }: { script = ''echo $((1 + $(dirname ${lib.escapeShellArg filePath})))''; }",
        r#"{ lib, filePath }: { script = ''sh -c "cat '${lib.escapeShellArg filePath}'"''; }"#,
        r"{ lib, filePath }: { script = ''bash -c 'mkdir -p $(dirname ${lib.escapeShellArg filePath})' ''; }",
        r#"{ lib, filePath, unknown }: { script = "echo ${unknown}; cat '${lib.escapeShellArg filePath}'"; }"#,
        r#"{ lib, filePath, unknown }: { script = "echo ${unknown}; mkdir -p $(dirname ${lib.escapeShellArg filePath})"; }"#,
        r#"{ lib, x, y }: { script = "echo ${lib.escapeShellArgs [ x y ]}; mkdir -p $(dirname ${lib.escapeShellArg x})"; }"#,
        r#"{ lib, filePath }: { script = "cat '${lib.escapeShellArg filePath}'; echo 'unterminated"; }"#,
        r#"{ lib, filePath }: { script = "cat '${lib.escapeShellArg filePath}'; echo \"unterminated"; }"#,
        r#"{ lib, filePath }: { script = "cat '${lib.escapeShellArg filePath}'; echo trailing\\"; }"#,
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
    // A preceding genuine code33 hole is still reported; it does not make
    // the later command substitution's lexical context trustworthy.
    assert_hint(
        r#"{ lib, filePath }: { script = "echo '${lib.escapeShellArg filePath}'; mkdir -p $(dirname ${lib.escapeShellArg filePath})"; }"#,
        "shell_double_escaping",
        33,
    );
}

#[test]
fn complex_or_non_argument_dirname_substitutions_are_skipped() {
    for source in [
        r#"{ script = ''$(dirname "$file")''; }"#,
        r#"{ script = ''echo prefix$(dirname "$file")''; }"#,
        r#"{ script = ''echo $(dirname "$file")suffix''; }"#,
        r#"{ script = ''echo $(dirname -z "$file")''; }"#,
        r"{ script = ''echo $(dirname first second)''; }",
        r"{ script = ''echo $(dirname)''; }",
        r#"{ script = ''echo $(dirname "$file"; printf extra)''; }"#,
        r#"{ script = ''echo $(dirname "$file";)''; }"#,
        r"{ script = ''echo $(dirname $(printf file))''; }",
        r"{ filePath }: { script = ''echo $(dirname ${filePath})''; }",
        r#"{ script = ''echo $(dirname "$file"''; }"#,
    ] {
        assert!(reports(source).is_empty(), "{source}");
    }
}
