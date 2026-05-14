{ inputs, ... }:
{
  imports = [ inputs.files.flakeModules.default ];
  perSystem = psArgs: {
    make-shells.default.packages = [ psArgs.config.files.writer.drv ];
  };
}
