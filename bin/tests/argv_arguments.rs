use lib::{LINTS, Report, Severity};

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
                .filter(move |lint| {
                    lint.name() == "argv_multi_flag_string" && lint.match_with(&kind)
                })
                .filter_map(move |lint| lint.validate(&node.clone().into()))
        })
        .collect()
}

fn assert_advisories(source: &str, arguments: &[&str]) {
    let reports = reports(source);
    assert_eq!(reports.len(), arguments.len(), "{source}");
    let mut unchanged = source.to_owned();
    for (report, argument) in reports.iter().zip(arguments) {
        assert_eq!(report.code, 32, "{source}");
        assert!(matches!(report.severity, Severity::Hint), "{source}");
        assert_eq!(report.diagnostics.len(), 1, "{source}");
        let diagnostic = &report.diagnostics[0];
        let range = usize::from(diagnostic.at.start())..usize::from(diagnostic.at.end());
        assert_eq!(&source[range], *argument, "{source}");
        assert!(diagnostic.suggestion.is_none(), "{source}");
        report.apply(&mut unchanged);
    }
    assert_eq!(unchanged, source, "advice must not split arguments");
}

fn assert_clean(source: &str) {
    assert!(reports(source).is_empty(), "{source}");
}

#[test]
fn detects_multiple_flags_inside_one_literal_argument() {
    assert_advisories(
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "clean" "user" "--keep 5 --keep-since 3d" ]; }"#,
        &[r#""--keep 5 --keep-since 3d""#],
    );
    assert_advisories(
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--keep=5 --keep-since=3d" "-v -q" ]; }"#,
        &[r#""--keep=5 --keep-since=3d""#, r#""-v -q""#],
    );
    assert_advisories(
        r#"{ launchd.agents.example.config.ProgramArguments = [ ("nh") (("--keep 5 --keep-since 3d")) ]; }"#,
        &[r#""--keep 5 --keep-since 3d""#],
    );
}

#[test]
fn separate_arguments_and_single_space_containing_values_are_valid() {
    for source in [
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "clean" "user" "--keep" "5" "--keep-since" "3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--keep-one" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "1 day" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--keep-since 1 day" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--label=hello world" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--threshold -5" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--name prefix--suffix" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--label=--debug" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--name --" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "--odd executable --name" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" " --keep 5 --keep-since 3d" ]; }"#,
    ] {
        assert_clean(source);
    }
    // A later end-of-options marker does not turn an earlier flag into a positional.
    assert_advisories(
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" "--" "operand" ]; }"#,
        &[r#""--keep 5 --keep-since 3d""#],
    );
}

#[test]
fn shell_and_interpreter_command_strings_are_not_argv_flag_lists() {
    for source in [
        r#"{ launchd.agents.example.config.ProgramArguments = [ "sh" "-c" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "/bin/bash" "-lc" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "/bin/zsh" "-c" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "eval" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "python3" "-c" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "/usr/bin/python3.12" "-c" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "perl" "-e" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "ruby" "-e" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "node" "--eval" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nix" "eval" "--expr" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "jq" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "/usr/bin/env" "bash" "-c" "--keep 5 --keep-since 3d" ]; }"#,
    ] {
        assert_clean(source);
    }
}

#[test]
fn does_not_assume_unrecognized_consumers_have_launchd_semantics() {
    for source in [
        r#"[ "nh" "--keep 5 --keep-since 3d" ]"#,
        r#"{ ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ custom.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.serviceConfig.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.OtherArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments.extra = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ services.example.arguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ programs.nh.clean.extraArgs = "--keep 5 --keep-since 3d"; }"#,
        r#"{ documentation = "--keep 5 --keep-since 3d"; }"#,
        r#"{ documentation.launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ documentation = { launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; }"#,
        r#"consume { launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"let documentation = { launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; in documentation"#,
    ] {
        assert_clean(source);
    }
}

#[test]
fn ambiguous_strings_and_dynamic_values_are_not_interpreted() {
    for source in [
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--keep ${count} --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "${flags}" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "${pkgs.nh}/bin/nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--label '--keep 5 --keep-since 3d'" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--label \"--keep 5 --keep-since 3d\"" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--label \"--debug\" --keep 5" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--label escaped\\ --keep 5" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--label escaped\ --keep 5" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5\n--keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--label {\"flag\": \"--debug\"}" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" ''--keep 5 --keep-since 3d'' ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" ("--keep 5" + " --keep-since 3d") ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ executable "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" subcommand "--keep 5 --keep-since 3d" ]; }"#,
        r"{ launchd.agents.example.config.ProgramArguments = args; }",
        r#"{ launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ] ++ extra; }"#,
        r#"{ launchd.agents.example.config.ProgramArguments = lib.optionals enabled [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.${agent}.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config.${key} = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ launchd.agents.example.config."Program\Arguments" = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
    ] {
        assert_clean(source);
    }
}

#[test]
fn equivalent_nested_literal_attribute_paths_are_recognized() {
    for source in [
        r#"{ launchd = { agents = { example = { config = { ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; }; }; }; }"#,
        r#"{ launchd.agents = { example.config = { ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; }; }"#,
        r#"{ launchd = ({ agents.example.config.ProgramArguments = ([ "nh" "--keep 5 --keep-since 3d" ]); }); }"#,
        r#"{ "launchd"."agents"."example-agent"."config"."ProgramArguments" = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ config.launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ config = { launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; }"#,
        r#"{ config = { launchd = { agents.example = { config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; }; }; }"#,
        r#"{ pkgs, ... }: ({ launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; })"#,
        r#"{ pkgs, ... }: let unrelated = true; in ({ config.launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; })"#,
    ] {
        assert_advisories(source, &[r#""--keep 5 --keep-since 3d""#]);
    }
    for source in [
        r#"{ config.config.launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }"#,
        r#"{ config = { config = { launchd.agents.example.config.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; }; }"#,
        r#"{ launchd.agents.example.config = consume { ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; }"#,
        r#"{ launchd.agents.example.config = let x = true; in { ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; }"#,
        r#"{ launchd.agents.example.config = { nested.ProgramArguments = [ "nh" "--keep 5 --keep-since 3d" ]; }; }"#,
    ] {
        assert_clean(source);
    }
}
