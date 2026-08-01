{ inputs, root, ... }:
{
  perSystem =
    { system, ... }:
    {
      imports = [ "${inputs.nixpkgs}/nixos/modules/misc/nixpkgs.nix" ];
      nixpkgs.hostPlatform = { inherit system; };
      nixpkgs.overlays = [ (import (root + "/overlay.nix")) ];
    };
}
