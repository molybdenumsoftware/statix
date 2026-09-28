mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: ruff,
    expressions: [
        "pkgs.writers.writePython3Bin \"hello\" { } ''\n  import os\n  print(\"hello\")\n''",
        "{ scripts.py = { package = pkgs.python312; exec = ''\n  import sys\n  print(${toString port})\n''; }; }",
        "pkgs.writeScript \"x\" ''\n  #!${pkgs.python3}/bin/python3\n  x == 1\n''",
        // clean
        "pkgs.writers.writePython3 \"ok\" { } ''\n  print(\"ok\")\n''",
    ],
}
