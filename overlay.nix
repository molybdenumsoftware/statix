final: _prev: {
  statix = final.rustPlatform.buildRustPackage {
    pname = "statix";
    version = "0.6.0-git";
    src = final.lib.fileset.toSource {
      root = ./.;
      fileset = final.lib.fileset.unions [
        (final.lib.fileset.fileFilter (
          file:
          final.lib.any final.lib.id [
            (file.name == "Cargo.toml")
            (file.hasExt "rs")
            (file.hasExt "snap")
          ]
        ) ./.)
        ./Cargo.lock
        ./insta.yaml
      ];
    };
    cargoLock.lockFile = ./Cargo.lock;
    buildFeatures = [ "json" ];
    meta = {
      mainProgram = "statix";
      description = "Lints and suggestions for the Nix programming language";
      homepage = "https://git.peppe.rs/languages/statix/about";
      license = final.lib.licenses.mit;
    };
  };

  statix-vim =
    let
      pluginRoot = ./vim-plugin;
    in
    final.vimUtils.buildVimPlugin {
      pname = "statix-vim";
      version = "0.1.0-git";
      src = final.lib.fileset.toSource {
        root = pluginRoot;
        fileset = final.lib.fileset.union (pluginRoot + "/plugin/statix.vim") (
          pluginRoot + "/ftplugin/nix.vim"
        );
      };
    };
}
