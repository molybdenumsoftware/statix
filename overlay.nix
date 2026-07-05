final: prev: {
  statix = prev.callPackage ./packages/statix.nix { };
  statix-vim = prev.callPackage ./packages/statix-vim.nix { };
}
