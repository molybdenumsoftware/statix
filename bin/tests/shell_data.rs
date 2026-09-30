use lib::{LINTS, Report, Severity};

const ADVISORIES: &[&str] = &["shell_unescaped_env", "shell_variable_interpolation"];

fn reports(source: &str) -> Vec<Report> {
    let parsed = rnix::Root::parse(source);
    assert!(
        parsed.errors().is_empty(),
        "invalid fixture: {:?}",
        parsed.errors()
    );
    parsed
        .syntax()
        .descendants()
        .flat_map(|node| {
            let kind = node.kind();
            LINTS
                .iter()
                .filter(move |lint| ADVISORIES.contains(&lint.name()) && lint.match_with(&kind))
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
    assert_eq!(reports[0].diagnostics.len(), 1, "{source}");
    assert!(reports[0].diagnostics[0].suggestion.is_none(), "{source}");
    let mut unchanged = source.to_owned();
    reports[0].apply(&mut unchanged);
    assert_eq!(
        unchanged, source,
        "shell advisories must not rewrite strings"
    );
}

fn assert_clean(source: &str) {
    assert!(reports(source).is_empty(), "{source}");
}

#[test]
fn exported_string_conversion_is_conditional_and_never_rewritten() {
    for source in [
        // Maintainer evidence: https://github.com/nix-community/home-manager/pull/9046
        r"{ lib, repository }: { script = ''export RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: { script = ''export RESTIC_REPOSITORY=${builtins.toString repository}''; }",
        r"{ repository }: { preBuild = ''export _REPOSITORY2=${((builtins).toString) (repository)}''; }",
        r"{ repository }: { script = ''printf '%s\n' ready; export RESTIC_REPOSITORY=${(toString repository)}''; }",
        r"{ repository }: { script = ''export FIRST=plain RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: { script = ''for x in one; do export RESTIC_REPOSITORY=${toString repository}; done''; }",
        r#"{ repository }: { script = "export RESTIC_REPOSITORY=${toString repository}"; }"#,
        r#"{ pkgs, repository }: pkgs.writeShellScript "backup" ''export RESTIC_REPOSITORY=${toString repository}''"#,
    ] {
        assert_hint(source, "shell_unescaped_env", 35);
    }
}

#[test]
fn export_recognition_requires_an_unquoted_assignment_and_shell_consumer() {
    for source in [
        r#"{ repository }: { script = ''export RESTIC_REPOSITORY="${toString repository}"''; }"#,
        r"{ repository }: { script = ''export RESTIC_REPOSITORY='${toString repository}' ''; }",
        r"{ repository }: { script = ''export RESTIC_REPOSITORY='prefix'${toString repository}''; }",
        r"{ repository }: { script = ''export RESTIC_REPOSITORY=${toString repository}'suffix' ''; }",
        r#"{ repository }: { script = ''export RESTIC_REPOSITORY=${toString repository}"suffix" ''; }"#,
        r"{ repository, suffix }: { script = ''export RESTIC_REPOSITORY=${toString repository}${suffix}''; }",
        r#"{ repository }: { script = ''export "RESTIC_REPOSITORY="${toString repository}''; }"#,
        r"{ repository }: { script = ''echo ${toString repository}''; }",
        r"{ repository }: { script = ''echo export RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: { script = ''export ${toString repository}''; }",
        r"{ repository }: { script = ''export -x RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: { script = ''export NOT_AN_ASSIGNMENT RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: { script = ''export 2INVALID=${toString repository}''; }",
        r"{ repository }: { script = ''RESTIC_REPOSITORY=${toString repository} echo ok''; }",
        r"{ repository }: { script = ''export; RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: { script = ''export
            RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: { description = ''export RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: ''export RESTIC_REPOSITORY=${toString repository}''",
        r"{ repository }: { script = ''# export RESTIC_REPOSITORY=${toString repository}
            echo ok''; }",
        r"{ repository }: { script = ''echo $(export RESTIC_REPOSITORY=${toString repository})''; }",
        r"{ repository }: { script = ''bash -c 'export RESTIC_REPOSITORY=${toString repository}' ''; }",
        r"{ repository }: { script = ''cat <<EOF
            export RESTIC_REPOSITORY=${toString repository}
            EOF''; }",
    ] {
        assert_clean(source);
    }
}

#[test]
fn exported_escaped_values_and_nonbuiltin_conversions_are_untouched() {
    for source in [
        r"{ lib, repository }: { script = ''export RESTIC_REPOSITORY=${lib.escapeShellArg repository}''; }",
        r"{ lib, repository }: { script = ''export RESTIC_REPOSITORY=${toString (lib.escapeShellArg repository)}''; }",
        r"{ toString, repository }: { script = ''export RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ builtins, repository }: { script = ''export RESTIC_REPOSITORY=${builtins.toString repository}''; }",
        r"{ repository }: let toString = value: value; in { script = ''export RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: let builtins = custom; in { script = ''export RESTIC_REPOSITORY=${builtins.toString repository}''; }",
        r"{ repository }: rec { toString = value: value; script = ''export RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: with custom; { script = ''export RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ repository }: with custom; { script = ''export RESTIC_REPOSITORY=${builtins.toString repository}''; }",
        r"{ repository }: { script = ''export RESTIC_REPOSITORY=${other.toString repository}''; }",
        r"{ repository }: { script = ''export RESTIC_REPOSITORY=${(builtins.toString or fallback) repository}''; }",
        r"{ repository }: { script = ''export RESTIC_REPOSITORY=${toString repository extra}''; }",
        r"{ prefix, repository }: { script = ''${prefix}; export RESTIC_REPOSITORY=${toString repository}''; }",
        r"{ prefix, repository }: { script = ''export FIRST=${prefix} RESTIC_REPOSITORY=${toString repository}''; }",
    ] {
        assert_clean(source);
    }
}

#[test]
fn root_shell_loop_variables_are_distinct_from_nix_interpolation() {
    for source in [
        // Escape guidance: https://discourse.nixos.org/t/dealing-with-shell-variables-in-nix-strings/7978/2
        r#"{ script = ''for x in one two; do printf '%s\n' "${x}"; done''; }"#,
        r"{ script = ''for x in one two; do printf '%s\n' ${x}; done''; }",
        r#"{ script = ''for _item2 in one; do echo "${_item2}"; done''; }"#,
        r#"{ script = ''for x in "one two"; do echo "${x}"; done''; }"#,
        r#"{ script = ''for x in one two
            do
                echo "${x}"
            done''; }"#,
        r#"{ script = "for x in one; do echo \"${x}\"; done"; }"#,
        r#"{ script = ''for x in one; do :; done; echo "${x}"''; }"#,
        r#"{ script = ''for y in one; do for x in two; do echo "${x}"; done; done''; }"#,
    ] {
        assert_hint(source, "shell_variable_interpolation", 36);
    }
}

#[test]
fn nix_supplied_identifiers_and_already_escaped_variables_are_not_guessed() {
    for source in [
        r#"{ script = ''for x in one two; do printf '%s\n' "''${x}"; done''; }"#,
        r#"{ script = ''for x in one two; do printf '%s\n' "$x"; done''; }"#,
        r#"{ script = "for x in one; do echo \"\${x}\"; done"; }"#,
        r#"x: { script = ''for x in one; do echo "${x}"; done''; }"#,
        r#"{ x }: { script = ''for x in one; do echo "${x}"; done''; }"#,
        r#"{ x ? "fallback" }: { script = ''for x in one; do echo "${x}"; done''; }"#,
        r"{ x, lib }: { script = ''for x in one; do echo ${lib.escapeShellArg x}; done''; }",
        r#"let x = "nix"; in { script = ''for x in one; do echo "${x}"; done''; }"#,
        r#"rec { x = "nix"; script = ''for x in one; do echo "${x}"; done''; }"#,
        r#"let inherit (input) x; in { script = ''for x in one; do echo "${x}"; done''; }"#,
        r#"with supplied; { script = ''for x in one; do echo "${x}"; done''; }"#,
        r#"{ script = ''for builtins in one; do echo "${builtins}"; done''; }"#,
        r#"{ script = ''for true in one; do echo "${true}"; done''; }"#,
        r#"{ script = ''for false in one; do echo "${false}"; done''; }"#,
        r#"{ script = ''for null in one; do echo "${null}"; done''; }"#,
        r#"{ script = ''for x in one; do echo "${(x)}"; done''; }"#,
        r#"{ script = ''for x in one; do echo "${object.x}"; done''; }"#,
    ] {
        assert_clean(source);
    }
}

#[test]
fn loop_recognition_uses_real_words_and_known_root_shell_scope() {
    for source in [
        r#"{ description = ''for x in one; do echo "${x}"; done''; }"#,
        r#"''for x in one; do echo "${x}"; done''"#,
        r#"{ script = ''echo 'for x in one; do'; echo "${x}"''; }"#,
        r#"{ script = ''echo "for x in one; do"; echo "${x}"''; }"#,
        r#"{ script = ''printf '%s\n' for x in one; do echo "${x}"; done''; }"#,
        r#"{ script = ''# for x in one; do
            echo "${x}"''; }"#,
        r"{ script = ''for x in one; do # echo ${x}
            :; done''; }",
        r"{ script = ''for x in one; do echo '${x}'; done''; }",
        r#"{ script = ''for x in one; do echo $(printf '%s\n' "${x}"); done''; }"#,
        r#"{ script = ''echo $(for x in one; do :; done); echo "${x}"''; }"#,
        r#"{ script = ''for y in one; do echo "${x}"; done''; }"#,
        r#"{ prefix }: { script = ''${prefix}; for x in one; do echo "${x}"; done''; }"#,
        r#"{ prefix }: { script = ''for x in one; do ${prefix}; echo "${x}"; done''; }"#,
        r#"{ script = ''bash -c 'for x in one; do echo "${x}"; done' ''; }"#,
    ] {
        assert_clean(source);
    }
}

#[test]
fn malformed_and_unsupported_loop_headers_are_not_inferred() {
    for source in [
        r#"{ script = ''for x; do echo "${x}"; done''; }"#,
        r#"{ script = ''for x in; do echo "${x}"; done''; }"#,
        r#"{ script = ''for 'x' in one; do echo "${x}"; done''; }"#,
        r#"{ script = ''for x "in" one; do echo "${x}"; done''; }"#,
        r#"{ script = ''for x from one; do echo "${x}"; done''; }"#,
        r#"{ script = ''for x in one do echo "${x}"; done''; }"#,
        r#"{ script = ''for x in one; echo "${x}"; done''; }"#,
        r#"{ script = ''for x in one && do echo "${x}"; done''; }"#,
        r#"{ script = ''for x in one || do echo "${x}"; done''; }"#,
        r#"{ script = ''for x in one | do echo "${x}"; done''; }"#,
        r#"{ script = ''printf '%s\n' one | for x in one; do :; done; echo "${x}"''; }"#,
        r#"{ script = ''true && for x in one; do echo "${x}"; done''; }"#,
        r#"{ script = ''for ((x=0; x<2; x++)); do echo "${x}"; done''; }"#,
        r#"{ script = ''while true; do for x in one; do echo "${x}"; done; done''; }"#,
        r#"{ script = ''if true; then for x in one; do echo "${x}"; done; fi''; }"#,
        r#"{ script = ''select x in one; do echo "${x}"; done''; }"#,
    ] {
        assert_clean(source);
    }
}
