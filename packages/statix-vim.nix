{
  vimUtils,
  lib,
}:
let
  pluginRoot = ../vim-plugin;
in
vimUtils.buildVimPlugin {
  pname = "statix-vim";
  version = "0.1.0-git";
  src = lib.fileset.toSource {
    root = pluginRoot;
    fileset = lib.fileset.union (pluginRoot + "/plugin/statix.vim") (pluginRoot + "/ftplugin/nix.vim");
  };
}
