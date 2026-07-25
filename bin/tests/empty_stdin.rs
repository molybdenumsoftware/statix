mod _utils;

#[test]
fn check_empty_stdin_reports_parse_error() {
    let stdout = _utils::test_cli_stdin("", &["check", "-s"]).unwrap();

    assert!(
        stdout.contains("[syntax_error] Error: Syntax error"),
        "{stdout}"
    );
    assert!(stdout.contains("Unexpected end of file"), "{stdout}");
    assert!(stdout.contains("<stdin>:1:1"), "{stdout}");
}
