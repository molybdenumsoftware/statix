mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: useless_has_attr,
    expressions: [
        // trivial
        "if x ? a then x.a else default\n",
        "if x.a ? b then x.a.b else default\n",
        "if x ? a.b then x.a.b else default\n",

        // complex body
        "if x ? a then x.a else if b then c else d\n",
        "if x ? a then x.a else b.c\n",
    ],
}
