{ lib, ... }:
{
  _module.args.license = lib.licenses.mit;
  perSystem.treefmt.settings.global.excludes = [ "LICENSE" ];
}
