final: prev: {
  statix-cargo-nix = prev.callPackage ./packages/cargo-nix.nix { };
  statix = prev.callPackage ./packages/statix.nix { };
  statix-workspace = prev.callPackage ./packages/statix-workspace.nix { };
  statix-vim = prev.callPackage ./packages/statix-vim.nix { };
}
