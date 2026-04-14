mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: bool_comparison,
    expressions: [
        // non-matches
        "i == j\n",
        "k != l\n",
        "a == false\n",
        "b == true\n",
        "false != c\n",
        "true == d\n",

        // matches
        "false != false\n",
        "false != true\n",
        "true != false\n",
        "true != true\n",
        "false == false\n",
        "false == true\n",
        "true == false\n",
        "true == true\n",
    ],
}
