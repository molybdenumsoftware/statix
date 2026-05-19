{
  nixConfig = {
    abort-on-warn = true;
    allow-import-from-derivation = false;
  };

  inputs = {
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        gitignore.follows = "gitignore_dedupe";
        flake-compat.follows = "flake-compat_dedupe";
      };
    };
    make-shell = {
      url = "github:nicknovitski/make-shell";
      inputs.flake-compat.follows = "flake-compat_dedupe";
    };
    files.url = "github:mightyiam/files";
    treefmt = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    gitignore_dedupe = {
      url = "github:hercules-ci/gitignore.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-compat_dedupe.url = "github:edolstra/flake-compat";
  };
  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } (
      { lib, ... }:
      {
        _module.args.root = ./.;

        imports = [
          ./docs/flake-part.nix
          ./flake-parts/cachix.nix
          ./flake-parts/ci.nix
          ./flake-parts/dependabot.nix
          ./flake-parts/dev-shell.nix
          ./flake-parts/dogfood.nix
          ./flake-parts/files.nix
          ./flake-parts/fmt.nix
          ./flake-parts/git-hooks.nix
          ./flake-parts/git-ignore.nix
          ./flake-parts/license.nix
          ./flake-parts/nixpkgs.nix
          ./flake-parts/rust.nix
          ./flake-parts/statix.nix
          ./flake-parts/systems.nix
          ./flake-parts/vim-plugin.nix
        ];
      }
    );
}
