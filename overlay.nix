inputs: final: prev: {
  statix = prev.callPackage ./packages/statix.nix { crate2nixSrc = inputs.crate2nix; };
  statix-vim = prev.callPackage ./packages/statix-vim.nix { };
}
