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

    import-tree = {
      url = "github:vic/import-tree";
      flake = false;
    };
    nixpkgs = {
      url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";
    };

    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      flake = false;
    };
    make-shell = {
      url = "github:nicknovitski/make-shell";
      flake = false;
    };
    files = {
      url = "github:mightyiam/files";
      flake = false;
    };
    treefmt = {
      url = "github:numtide/treefmt-nix";
      flake = false;
    };
  };
  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } (
      { lib, ... }:
      {
        _module.args.root = ./.;

        imports = [ (import inputs.import-tree ./flake-parts) ];
      }
    );
}
