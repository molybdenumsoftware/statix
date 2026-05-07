{ inputs, root, ... }:
{
  perSystem =
    { system, ... }:
    {
      _module.args.pkgs = import inputs.nixpkgs {
        inherit system;
        config = { };
        overlays = [ (import (root + "/overlay.nix")) ];
      };
    };
}
