{ inputs, ... }:
{
  perSystem = _: {
    nixpkgs.overlays = [
      (_: prev: {
        crate2nixTools = prev.callPackage "${inputs.crate2nix}/tools.nix" { };
      })
    ];
  };
}
