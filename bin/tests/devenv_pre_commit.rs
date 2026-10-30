mod _utils;

use macros::generate_tests;

generate_tests! {
    rule: devenv_pre_commit,
    expressions: [
        "{ pre-commit.hooks.shellcheck.enable = true; }",
        "{ pre-commit = { hooks.nixfmt.enable = true; }; }",
        "{ config.pre-commit.hooks.shellcheck.enable = true; }",
        // git-hooks.nix flake-parts module still uses `pre-commit.settings`
        "{ perSystem = { pre-commit.settings.hooks.nixfmt.enable = true; }; }",
        "{ pre-commit.settings.hooks.nixfmt.enable = true; }",
    ],
}
