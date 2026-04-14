mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: deprecated_to_path,
    expressions: [
        "builtins.toPath x\n",
        "toPath x\n",
        r#"toPath "/abc/def"
"#,
        r#"builtins.toPath "/some/path"
"#,
    ],
}
