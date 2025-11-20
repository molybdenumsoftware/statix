/*
  Native flake-parts compatibility layer for classical Nix workflows.
  Reads inputs from flake.lock, constructs flake structures, handles pure evaluation.
  Returns an attribute set of the shape: { packages, nixosConfigurations, ... }

  Usage examples:
    Direct import:       import /path/to/statix {}
    With npins override: let sources = import ./npins;
                         in import sources.statix { inputOverrides = { nixpkgs = sources.nixpkgs; }; }
    With niv:            let sources = import ./nix/sources.nix;
                         in import sources.statix { inputOverrides = { nixpkgs = sources.nixpkgs; }; }
    In NixOS config:     let statix = import /path/to/statix {};
                         in { environment.systemPackages = [ statix.packages.${pkgs.stdenv.hostPlatform.system}.default ]; }
*/
{
  src ? ./.,
  inputOverrides ? { },
}:
let
  lock = builtins.fromJSON (builtins.readFile (src + "/flake.lock"));

  inherit (builtins) mapAttrs listToAttrs;

  fetchInput =
    {
      owner,
      repo,
      rev,
      narHash,
      ...
    }:
    fetchTarball {
      url = "https://github.com/${owner}/${repo}/archive/${rev}.tar.gz";
      sha256 = narHash;
    };

  fetchedInputs = mapAttrs (
    _: nodeName: fetchInput lock.nodes.${nodeName}.locked
  ) lock.nodes.root.inputs;

  rawInputs = fetchedInputs // inputOverrides;

  lib = import "${rawInputs.nixpkgs}/lib";
  systemsList = import rawInputs.systems;

  inputs = {
    nixpkgs = {
      _type = "flake";
      outPath = rawInputs.nixpkgs;
      legacyPackages = listToAttrs (
        map (system: {
          name = system;
          value = import rawInputs.nixpkgs { inherit system; };
        }) systemsList
      );
    };

    flake-parts =
      let
        extrasDir = rawInputs.flake-parts + "/extras";
      in
      {
        outPath = rawInputs.flake-parts;
        lib = (import "${rawInputs.flake-parts}/lib.nix") { inherit lib; };
        flakeModules = lib.mapAttrs' (
          name: _: lib.nameValuePair (lib.removeSuffix ".nix" name) "${extrasDir}/${name}"
        ) (lib.filterAttrs (n: t: t == "regular" && lib.hasSuffix ".nix" n) (builtins.readDir extrasDir));
      };

    systems = rawInputs.systems;

    self = {
      outPath = src;
      inherit inputs;
    };
  };
in
(import (src + "/flake.nix")).outputs inputs
