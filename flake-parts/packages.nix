{
  perSystem = { pkgs, ... }: {
    packages = {
      default = pkgs.statix;
      inherit (pkgs) statix statix-vim;
    };
  };
}
