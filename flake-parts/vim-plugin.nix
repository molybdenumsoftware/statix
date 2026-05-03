{ lib, root, ... }:
{
  perSystem =
    { pkgs, ... }:
    let
      pluginRoot = root + "/vim-plugin";
    in
    {
      packages.statix-vim = pkgs.vimUtils.buildVimPlugin {
        pname = "statix-vim";
        version = "0.1.0-git";
        src = lib.fileset.toSource {
          root = pluginRoot;
          fileset = lib.fileset.union (pluginRoot + "/plugin/statix.vim") (pluginRoot + "/ftplugin/nix.vim");
        };
      };
    };

  partitions.dev.module.perSystem = psArgs: {
    treefmt.settings.global.excludes = [ "*.vim" ];
    checks."packages/statix-vim" = psArgs.config.packages.statix-vim;
  };
}
