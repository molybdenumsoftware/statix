mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: empty_list_concat,
    expressions: [
        // no match
        "[1 2] ++ [3 4]\n",

        // unnecessary left
        "[] ++ [1 2 3]\n",

        // unnecessary right
        "[1 2 3] ++ []\n",

        // collapses to a single array
        "[] ++ []\n",

        // multiple empties
        "[] ++ [] ++ []\n",
    ],
}
