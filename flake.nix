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
  };
  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } (
      { lib, ... }:
      {
        _module.args.root = ./.;

        imports = [
          inputs.flake-parts.flakeModules.partitions
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
          ./flake-parts/overlay.nix
          ./flake-parts/rust.nix
          ./flake-parts/statix.nix
          ./flake-parts/systems.nix
          ./vim-plugin/flake-part.nix
        ];

        partitionedAttrs = lib.genAttrs [
          "checks"
          "apps"
        ] (_: "dev");

        partitions.dev.extraInputsFlake = ./dev-flake;
      }
    );
}
