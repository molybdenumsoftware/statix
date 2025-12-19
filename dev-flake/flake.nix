{
  inputs = {
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs = {
        flake-compat.follows = "flake-compat_dedupe";
        gitignore.follows = "gitignore_dedupe";
        nixpkgs.follows = "nixpkgs_dedupe";
      };
    };
    make-shell = {
      url = "github:nicknovitski/make-shell";
      inputs.flake-compat.follows = "flake-compat_dedupe";
    };
    nixpkgs_dedupe.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";
    files.url = "github:mightyiam/files";
    flake-compat_dedupe.url = "github:edolstra/flake-compat";
    gitignore_dedupe = {
      url = "github:hercules-ci/gitignore.nix";
      inputs.nixpkgs.follows = "nixpkgs_dedupe";
    };
    treefmt = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs_dedupe";
    };
  };
  outputs = _: { };
}
