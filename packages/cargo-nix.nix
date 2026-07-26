{
  lib,
  crate2nixTools,
}:
crate2nixTools.generatedCargoNix {
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
}
