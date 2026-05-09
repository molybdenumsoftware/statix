{
  partitions.dev.module.perSystem =
    { pkgs, ... }:
    {
      treefmt.settings.global.excludes = [ "*.vim" ];
      checks."statix-vim" = pkgs.statix-vim;
    };
}
