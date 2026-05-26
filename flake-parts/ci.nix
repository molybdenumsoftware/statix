{ lib, ... }:
let
  filePaths = {
    check = ".github/workflows/check.yaml";
  };

  ids = {
    jobs = {
      getCheckNames = "get-check-names";
      check = "check";
    };
    steps = {
      getCheckNames = "get-check-names";
      appToken = "app-token";
    };
    outputs = {
      jobs.getCheckNames = "checks";
      steps.getCheckNames = "checks";
    };
  };

  matrixParam = "checks";

  nixArgs = "--accept-flake-config";

  runner = {
    name = "ubuntu-latest";
    system = "x86_64-linux";
  };

  steps = {
    checkout.uses = "actions/checkout@v4";
    installNix = {
      uses = "nixbuild/nix-quick-install-action@master";
      "with" = {
        nix_conf = ''
          keep-env-derivations = true
          keep-outputs = true
        '';
        github_access_token = "\${{ secrets.GITHUB_TOKEN }}";
      };
    };
    cacheNix = {
      uses = "nix-community/cache-nix-action@main";
      "with".primary-key = "nix-\${{ runner.os }}";
    };
  };
in
{
  perSystem =
    { pkgs, ... }:
    {
      files.file.${filePaths.check}.source = pkgs.writers.writeJSON "gh-actions-workflow-check.yaml" {
        name = "Check";
        on = {
          pull_request = { };
          push = { };
          workflow_dispatch = { };
        };
        jobs = {
          ${ids.jobs.getCheckNames} = {
            runs-on = runner.name;
            outputs.${ids.outputs.jobs.getCheckNames} =
              "\${{ steps.${ids.steps.getCheckNames}.outputs.${ids.outputs.steps.getCheckNames} }}";
            steps = [
              steps.checkout
              steps.installNix
              steps.cacheNix
              {
                id = ids.steps.getCheckNames;
                run = ''
                  checks="$(nix ${nixArgs} eval --json .#checks.${runner.system} --apply builtins.attrNames)"
                  echo "${ids.outputs.steps.getCheckNames}=$checks" >> $GITHUB_OUTPUT
                '';
              }
            ];
          };

          ${ids.jobs.check} = {
            needs = ids.jobs.getCheckNames;
            runs-on = runner.name;
            strategy.matrix.${matrixParam} =
              "\${{ fromJson(needs.${ids.jobs.getCheckNames}.outputs.${ids.outputs.jobs.getCheckNames}) }}";
            steps = [
              steps.checkout
              steps.installNix
              steps.cacheNix
              {
                run = ''
                  nix ${nixArgs} build '.#checks.${runner.system}."''${{ matrix.${matrixParam} }}"'
                '';
              }
            ];
          };
        };
      };

      treefmt.settings.global.excludes = lib.attrValues filePaths;
    };
}
