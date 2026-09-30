mod _utils;

#[test]
fn removes_only_inert_backslashes_across_fix_passes() {
    for (source, expected) in [
        (
            r#""Library/Application\ Support/go""#,
            r#""Library/Application Support/go""#,
        ),
        (r#""a\ b\ c""#, r#""a b c""#),
        (r#""a\\\ b""#, r#""a\\ b""#),
        (
            r#"x: "left\ ${x}right\ side""#,
            r#"x: "left ${x}right side""#,
        ),
    ] {
        let output = _utils::test_cli_stdin(source, &["fix", "--stdin"]).unwrap();
        assert_eq!(output.trim_end(), expected);
        assert!(rnix::Root::parse(output.trim_end()).errors().is_empty());
        let warnings =
            _utils::test_cli_stdin(expected, &["check", "--stdin", "-o", "json"]).unwrap();
        assert!(warnings.is_empty(), "{warnings}");
    }
}

#[test]
fn preserves_real_backslashes_valid_escapes_and_indented_strings() {
    for source in [
        r#""literal\\ space""#,
        r#""line\nnext\tcolumn\rreturn""#,
        r#""escaped\"quote and \${interpolation}""#,
        "''Application\\ Support''",
        r#""unicode λ\\ space""#,
    ] {
        let output = _utils::test_cli_stdin(source, &["fix", "--stdin"]).unwrap();
        assert_eq!(output.trim_end(), source);
        let warnings = _utils::test_cli_stdin(source, &["check", "--stdin", "-o", "json"]).unwrap();
        assert!(warnings.is_empty(), "{warnings}");
    }
}

#[test]
fn reports_ineffective_escape_at_its_backslash() {
    let source = r#""Application\ Support""#;
    let output = _utils::test_cli_stdin(source, &["check", "--stdin", "-o", "json"]).unwrap();
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["report"][0]["name"], "ineffective_string_escape");
    assert_eq!(
        report["report"][0]["diagnostics"][0]["suggestion"]["fix"],
        ""
    );
    assert_eq!(
        report["report"][0]["diagnostics"][0]["at"]["from"]["column"],
        13
    );
}
