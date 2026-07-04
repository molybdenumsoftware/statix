final: _prev: {
  statix = final.callPackage ./packages/statix.nix { };
  statix-vim = final.callPackage ./packages/statix-vim.nix { };
}
