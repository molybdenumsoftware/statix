{
  lib,
  callPackage,
  defaultCrateOverrides,
  crate2nixTools,
  gitMinimal,
}:
let
  cargoNix = crate2nixTools.generatedCargoNix {
    name = "statix";
    src = lib.fileset.toSource {
      root = ../.;
      fileset = lib.fileset.unions [
        (lib.fileset.fileFilter (
          file:
          lib.any lib.id [
            (file.name == "Cargo.toml")
            (file.hasExt "rs")
            (file.hasExt "snap")
          ]
        ) ../.)
        ../Cargo.lock
        ../insta.yaml
      ];
    };
  };
  built = (callPackage cargoNix { }).workspaceMembers.statix.build.override {
    runTests = true;
    testInputs = [ gitMinimal ];
    crateOverrides = defaultCrateOverrides // {
      statix = _: {
        useClippy = true;
        capLints = "forbid";
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
