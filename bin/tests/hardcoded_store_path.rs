mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: hardcoded_store_path,
    expressions: [
        r#""/nix/store/q2l6gv5cdyx2ayx6frgz2rmd0mp4q6sw-hello-2.12.1/bin/hello""#,
        "''\n  cat /nix/store/q2l6gv5cdyx2ayx6frgz2rmd0mp4q6sw-hello-2.12.1/share/x; echo\n''",
        "/nix/store/q2l6gv5cdyx2ayx6frgz2rmd0mp4q6sw-hello-2.12.1/bin/hello",
        // not a store path
        r#""/nix/store/.links""#,
        r#""${pkgs.hello}/bin/hello""#,
    ],
}
