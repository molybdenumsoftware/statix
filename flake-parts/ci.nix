let
  path = ".github/workflows/check.yaml";
in
{
  perSystem =
    { pkgs, ... }:
    {
      files.file.${path}.source = pkgs.writers.writeJSON "gh-actions-workflow-check.yaml" {
        name = "Check";
        on = {
          pull_request = { };
          push = { };
          workflow_dispatch = { };
        };
        jobs = {
          check = {
            runs-on = "ubuntu-latest";
            steps = [
              { uses = "actions/checkout@v5"; }
              {
                uses = "cachix/install-nix-action@master";
                "with" = {
                  extra_nix_config = ''
                    keep-env-derivations = true
                    keep-outputs = true
                  '';
                  github_access_token = "\${{ secrets.GITHUB_TOKEN }}";
                };
              }
              # error: opening lock file "/nix/var/nix/temproots/2926": Permission denied
              # https://github.com/molybdenumsoftware/statix/actions/runs/27678513502/job/81859681633?pr=2671#step:5:9
              # {
              #   uses = "nix-community/cache-nix-action@v7";
              #   "with".primary-key = "nix-\${{ runner.os }}";
              # }
              {
                run = "nix --accept-flake-config flake check --print-build-logs";
              }
            ];
          };
        };
      };

      treefmt.settings.global.excludes = [ path ];
    };
}
