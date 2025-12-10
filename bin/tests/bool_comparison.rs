mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: bool_comparison,
    expressions: [
        // non-matches
        "i == j",
        "k != l",
        "a == false",
        "b == true",
        "false != c",
        "true == d",

        // matches
        "false != false",
        "false != true",
        "true != false",
        "true != true",
        "false == false",
        "false == true",
        "true == false",
        "true == true",
    ],
}
