mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: shellcheck,
    expressions: [
        "{ scripts.hello.exec = ''\n  echo $1\n''; }",
        r#"{ processes.web.exec = "cd $HOME && serve"; }"#,
        "{ enterShell = ''\n  echo ''${GREETING}\n''; }",
        r#"{ tasks."app:run".exec = "${pkgs.hello}/bin/hello $ARGS"; }"#,
        "{ packages = [ (pkgs.writeShellScriptBin \"list\" ''\n  for f in $(ls *.txt); do echo \"$f\"; done\n'') ]; }",
        "pkgs.writeShellApplication { name = \"x\"; text = ''\n  rm -rf \"$DIR\"/*\n''; }",
        // built-in fixes: read -r, declare and assign separately (keeping `${...}`)
        "{ tasks.\"a:b\".exec = ''\n  read name\n  export VERSION=$(${pkgs.git}/bin/git describe)\n  echo \"$name $VERSION\"\n''; }",
        // let bindings: used as a script, and interpolated into one as a fragment
        "let greet = \"echo $1\"; setup = ''\n  cd $HOME\n''; in { scripts.greet.exec = greet; enterShell = ''\n  ${setup}\n  echo ready\n''; }",
        // placeholder artifacts are not reported
        "{ scripts.all.exec = ''\n  for p in ${lib.escapeShellArgs platforms}; do echo \"$p\"; done\n''; }",
        // not shell
        "{ scripts.py = { package = pkgs.python3; exec = ''\n  print($1)\n''; }; }",
        "{ scripts.py.exec = ''\n  #!/usr/bin/env python3\n  print($1)\n''; }",
        "{ description = \"echo $1\"; }",
        // clean
        "{ scripts.ok.exec = ''\n  echo \"$1\"\n''; }",
    ],
}

#[test]
fn agent_format_explains_what_needs_manual_work() {
    let expr = "{ scripts.hello.exec = ''\n  echo $1\n  touch $(date +%s).log\n''; }\n";
    let stdout = _utils::test_cli(expr, &["check", "--format", "agent"]).unwrap();
    insta::assert_snapshot!(stdout);
}

generate_tests! {
    rule: shellcheck,
    expressions: [
        // stdenv phases: $out, $src ... come from stdenv, not reported
        "stdenv.mkDerivation { installPhase = ''\n  mkdir -p $out/bin\n  cp $src/tool $out/bin/\n''; }",
        "runCommand \"x\" { } ''\n  touch $out\n  cd $TMPDIR\n''",
        "{ systemd.services.web = { script = ''\n  cd $STATE_DIRECTORY\n''; }; }",
        "{ config = mkIf cfg.enable { systemd.services.${name}.preStart = ''\n  rm -rf $CACHE/*\n''; }; }",
    ],
}
