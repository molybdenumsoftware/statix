{
  rustPlatform,
  lib,
  clippy,
}:
rustPlatform.buildRustPackage {
  pname = "statix";
  version = "0.6.0-git";
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
  RUSTFLAGS = "-D warnings";

  nativeBuildInputs = [ clippy ];

  checkPhase = ''
    runHook preCheck

    cargo clippy --all-targets --all-features
    cargoCheckHook

    runHook postCheck
  '';

  cargoLock.lockFile = ../Cargo.lock;
  meta = {
    mainProgram = "statix";
    description = "Lints and suggestions for the Nix programming language";
    homepage = "https://github.com/molybdenumsoftware/statix";
    license = lib.licenses.mit;
  };
}
