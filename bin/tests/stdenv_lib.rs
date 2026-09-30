mod _utils;

#[test]
fn fixes_only_with_an_existing_unredefined_lib_argument() {
    let source = "{ lib, stdenv }: stdenv.lib.licenses.mit";
    let fixed = _utils::test_cli_stdin(source, &["fix", "--stdin"]).unwrap();
    assert_eq!(fixed.trim_end(), "{ lib, stdenv }: lib.licenses.mit");
    assert!(rnix::Root::parse(&fixed).errors().is_empty());
}

#[test]
fn warns_without_adding_arguments_or_deleting_comments() {
    for source in [
        "{ stdenv }: stdenv.lib.licenses.mit",
        "{ stdenv }: let lib = {}; in stdenv.lib.licenses.mit",
        "{ lib, stdenv }: stdenv /* keep */ .lib.licenses.mit",
    ] {
        let fixed = _utils::test_cli_stdin(source, &["fix", "--stdin"]).unwrap();
        assert_eq!(fixed.trim_end(), source);
        let output = _utils::test_cli_stdin(source, &["check", "--stdin", "-o", "json"]).unwrap();
        let report: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(report["report"][0]["name"], "deprecated_stdenv_lib");
        assert!(report["report"][0]["diagnostics"][0]["suggestion"].is_null());
    }
}

#[test]
fn ignores_package_outputs_local_stdenv_and_selection_defaults() {
    for source in [
        "stdenv: stdenv.cc.cc.lib",
        "stdenv: stdenv.libc",
        "let stdenv = { lib = { value = 1; }; }; in stdenv.lib.value",
        "{ lib, stdenv }: stdenv.lib or {}",
    ] {
        let output = _utils::test_cli_stdin(source, &["check", "--stdin", "-o", "json"]).unwrap();
        assert!(output.is_empty(), "{output}");
    }
}

#[test]
fn defaulted_inputs_never_trigger_a_library_replacement() {
    let rule = lib::LINTS
        .iter()
        .find(|rule| rule.name() == "deprecated_stdenv_lib")
        .unwrap();
    for (source, expected_reports) in [
        ("{ lib, stdenv ? custom }: stdenv.lib.licenses.mit", 0),
        ("{ lib ? custom, stdenv }: stdenv.lib.licenses.mit", 1),
    ] {
        let parsed = rnix::Root::parse(source);
        assert!(parsed.errors().is_empty());
        let reports: Vec<_> = parsed
            .syntax()
            .descendants()
            .filter(|node| rule.match_with(&node.kind()))
            .filter_map(|node| rule.validate(&node.into()))
            .collect();
        assert_eq!(reports.len(), expected_reports, "{source}");
        let mut unchanged = source.to_owned();
        for report in reports {
            assert!(
                report
                    .diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.suggestion.is_none())
            );
            report.apply(&mut unchanged);
        }
        assert_eq!(unchanged, source);
    }
}
