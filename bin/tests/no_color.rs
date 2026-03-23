mod _utils;

use crate::_utils::OutputProcessing;
use std::collections::HashMap;

const EXPR: &'static str = "let in null";

#[test]
fn test_output_with_color_ansi_escape() {
    let stdout = _utils::test_cli(
        EXPR,
        &["check"],
        HashMap::new(),
        OutputProcessing::Unchanged,
    )
    .unwrap();
    let snapshot = format!("{EXPR}\n---\n{stdout}");
    insta::with_settings!({omit_expression => true}, {
        insta::assert_snapshot!("output_with_color_ansi_escape", snapshot);
    });
}

#[test]
fn test_output_without_color_ansi_escape() {
    let stdout = _utils::test_cli(
        EXPR,
        &["check"],
        HashMap::from([("NO_COLOR", "1")]),
        OutputProcessing::Unchanged,
    )
    .unwrap();
    let snapshot = format!("{EXPR}\n---\n{stdout}");
    insta::with_settings!({omit_expression => true}, {
        insta::assert_snapshot!("output_without_color_ansi_escape", snapshot);
    });
}
