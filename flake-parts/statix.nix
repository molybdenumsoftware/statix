{
  root,
  ...
}:
{
  perSystem =
    { pkgs, ... }:
    let
      statixBuild = pkgs.statix-workspace.workspaceMembers.statix.build;
    in
    {
      treefmt.settings.global.excludes = [ "bin/tests/data/*.nix" ];
      checks."statix" = pkgs.statix;
      checks."statix-tests" = statixBuild.override {
        runTests = true;
        testPreRun = ''
          mkdir -p $TMPDIR/cargo-wrapper/bin
          cat > $TMPDIR/cargo-wrapper/bin/cargo << 'WRAPPER'
          #!/bin/sh
          if [ "$1" = "run" ] && [ "$2" = "--" ]; then
            shift 2
            exec ${pkgs.statix}/bin/statix "$@"
          fi
          exec ${pkgs.cargo}/bin/cargo "$@"
          WRAPPER
          chmod +x $TMPDIR/cargo-wrapper/bin/cargo
          export PATH=$TMPDIR/cargo-wrapper/bin:${pkgs.cargo}/bin:$PATH
          export INSTA_SNAPSHOT_DIR=${root}/bin/tests/snapshots
        '';
      };
    };
}
