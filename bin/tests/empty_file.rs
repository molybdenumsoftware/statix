mod _utils;

#[test]
fn check_empty_file_reports_parse_error() {
    let stdout = _utils::test_cli("", &["check"]).unwrap();

    assert!(stdout.contains("[00] Error: Syntax error"), "{stdout}");
    assert!(stdout.contains("Unexpected end of file"), "{stdout}");
    assert!(stdout.contains("<temp_file_path>:1:1"), "{stdout}");
}
