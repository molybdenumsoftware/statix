{ inputs, ... }:
{
  imports = [ "${inputs.files}/flake-module.nix" ];
  perSystem = psArgs: {
    make-shells.default.packages = [ psArgs.config.files.writer.drv ];
  };
}
