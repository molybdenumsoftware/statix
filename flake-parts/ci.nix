{ lib, ... }:
let
  filePaths = {
    check = ".github/workflows/check.yaml";
    updateFlakeInputs = ".github/workflows/update-flake-inputs.yaml";
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
  partitions.dev.module = {
    perSystem =
      { pkgs, ... }:
      {
        files.files = [
          {
            path_ = filePaths.check;
            drv = pkgs.writers.writeJSON "gh-actions-workflow-check.yaml" {
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

                legacy = {
                  name = "Build statix via flake-compat and install it using `nix-env`";
                  runs-on = runner.name;
                  steps = [
                    steps.checkout
                    steps.installNix
                    steps.cacheNix
                    { run = "nix-env --install --file default.nix"; }
                  ];
                };
              };
            };
          }
          {
            path_ = filePaths.updateFlakeInputs;
            drv = pkgs.writers.writeJSON "update-flake-inputs.yaml" {
              name = "Update flake inputs";
              on = {
                workflow_dispatch = { };
                schedule = [ { cron = "0 0/6 * * *"; } ];
              };
              jobs.nix-flake-update = {
                permissions = {
                  contents = "write";
                  pull-requests = "write";
                };
                runs-on = runner.name;
                steps = [
                  {
                    id = ids.steps.appToken;
                    uses = "actions/create-github-app-token@v1";
                    "with" = {
                      app-id = "\${{ secrets.APP_ID }}";
                      private-key = "\${{ secrets.APP_PRIVATE_KEY }}";
                    };
                  }
                  (
                    steps.checkout
                    // {
                      "with".token = "\${{ steps.${ids.steps.appToken}.outputs.token }}";
                    }
                  )
                  steps.installNix
                  steps.cacheNix
                  {
                    uses = "mic92/update-flake-inputs@main";
                    "with" = {
                      github-token = "\${{ steps.${ids.steps.appToken}.outputs.token }}";
                      commit-message = "chore(flake): update {{input}}{{in}}";
                    };
                  }
                ];
              };
            };
          }
        ];

        treefmt.settings.global.excludes = lib.attrValues filePaths;
      };
  };
}
