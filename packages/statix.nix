{
  lib,
  callPackage,
  defaultCrateOverrides,
  statix-cargo-nix,
}:
let
  built = (callPackage statix-cargo-nix { }).workspaceMembers.statix.build.override {
    runTests = true;
    crateOverrides = defaultCrateOverrides // {
      statix = _: {
        useClippy = true;
        RUSTFLAGS = "-D warnings";
      };
    };
  };
in
built.overrideAttrs (_: {
  meta = {
    mainProgram = "statix";
    description = "Lints and suggestions for the Nix programming language";
    homepage = "https://github.com/molybdenumsoftware/statix";
    license = lib.licenses.mit;
  };
})
