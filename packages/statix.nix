{
  lib,
  pkgs,
  crate2nixSrc,
}:
let
  tools = pkgs.callPackage "${crate2nixSrc}/tools.nix" { };
  cargoNix = tools.generatedCargoNix {
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
in
(pkgs.callPackage "${cargoNix}/default.nix" { }).workspaceMembers.statix.build.override {
  features = [
    "default"
    "json"
  ];
}
