mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: bool_simplification,
    expressions: [
        "!(a == b)\n",

        // non-matches
        "!(a != b)\n",
        "a != b\n",
    ],
}
