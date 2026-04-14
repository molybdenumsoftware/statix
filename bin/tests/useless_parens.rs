mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: useless_parens,
    expressions: [
        // parens around primitives
        r#"("hello")
"#,
        "let b = 0; in (b)\n",
        "({ f = 2; })\n",

        // parens around let-value
        "let a = (1 + 2); in null\n",
        "let h = ({ inherit (builtins) map; }); in null\n",

        // LATER: binary exprs, function args etc.

        // parens around let body
        "let a = 0; in (null)\n",

        // select in list (parens not necessary)
        "[(builtins.map)]\n",
        "[(builtins.pam or map)]\n",
    ],
}
