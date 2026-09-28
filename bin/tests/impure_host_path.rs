mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: impure_host_path,
    expressions: [
        r#""/usr/local/bin/jq . data.json""#,
        "''\n  #!/bin/bash\n  /opt/homebrew/bin/brew install x\n  /usr/bin/python3 -V\n''",
        "/usr/local/bin/bash",
        // allowed
        r##""#!/usr/bin/env bash""##,
        r#""${pkgs.bash}/bin/bash""#,
    ],
}
