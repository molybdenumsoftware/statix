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
              { uses = "actions/checkout@v4"; }
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
              {
                uses = "nix-community/cache-nix-action@main";
                "with".primary-key = "nix-\${{ runner.os }}";
              }
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
