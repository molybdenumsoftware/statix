mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: empty_pattern,
    expressions: [
        // match
        "({ ... }: 42)\n",
        "({ ... } @ inputs: inputs)\n",

        // don't match
        "({ a, ... }: a)\n",

        // nixos module, don't match
        "({ ... }: { imports = []; })\n",
    ],
}
