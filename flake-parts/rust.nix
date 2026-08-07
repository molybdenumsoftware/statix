{ inputs, ... }:
{
  gitignore = [ "/target" ];
  perSystem =
    { pkgs, ... }:
    {
      make-shells.default = {
        packages = [
          pkgs.cargo
          pkgs.rustc
          pkgs.bacon
          pkgs.cargo-insta
          pkgs.rust-analyzer
        ];
        env = {
          RUST_LOG = "info";
          RUST_BACKTRACE = 1;
        };
      };
      treefmt = {
        programs.rustfmt.enable = true;
        settings.global.excludes = [
          "bin/tests/snapshots/*.snap"
        ];
      };
    };
}
