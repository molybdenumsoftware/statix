mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: devenv_exec_shebang,
    expressions: [
        "{ scripts.hello.exec = ''\n  #!/usr/bin/env python3\n  print(1)\n''; }",
        "{ config.tasks.\"app:setup\".exec = \"#!/bin/sh\\necho hi\"; }",
        "{ processes = { web = { exec = ''\n  #!/usr/bin/env bash\n  serve\n''; }; }; }",
        // interpreter set explicitly: shebang is a harmless comment
        "{ scripts.hello = { package = pkgs.python3; exec = ''\n  #!/usr/bin/env python3\n  print(1)\n''; }; }",
        // no shebang
        "{ scripts.hello.exec = \"echo hi\"; }",
        // not a devenv option
        "{ foo.bar.exec = \"#!/bin/sh\"; }",
    ],
}
