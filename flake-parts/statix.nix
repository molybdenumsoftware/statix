{
  perSystem =
    { pkgs, ... }:
    {
      treefmt.settings.global.excludes = [ "bin/tests/data/*.nix" ];
      checks."statix" = pkgs.statix;
    };
}
