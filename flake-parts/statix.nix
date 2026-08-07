{
  perSystem =
    { pkgs, ... }:
    {
      treefmt.settings.global.excludes = [ "bin/tests/data/*.nix" ];
      checks.build = pkgs.statix;
    };
}
