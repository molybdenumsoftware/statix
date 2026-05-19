{
  gitignore = [ "/target" ];
  perSystem =
    { pkgs, ... }:
    {
      make-shells.default = {
        inputsFrom = [ pkgs.statix ];
        packages = [
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
