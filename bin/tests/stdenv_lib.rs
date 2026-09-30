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
