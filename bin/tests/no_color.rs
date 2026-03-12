mod _utils;

use crate::_utils::OutputProcessing;
use std::collections::HashMap;

#[test]
fn test_no_colors() {
    let expr = "let in null";

    let empty_env: HashMap<&str, &str> = HashMap::new();
    let stdout =
        _utils::test_cli(expr, &["check"], OutputProcessing::Unchanged, empty_env).unwrap();
    insta::assert_snapshot!(
        format!("output_with_color_ansi_escape"),
        stdout,
        &format!("{expr:?}")
    );

    let no_color_env: HashMap<&str, &str> = HashMap::from([("NO_COLOR", "1")]);
    let stdout =
        _utils::test_cli(expr, &["check"], OutputProcessing::Unchanged, no_color_env).unwrap();
    insta::assert_snapshot!(
        format!("output_without_color_ansi_escape"),
        stdout,
        &format!("{expr:?}")
    );
}
