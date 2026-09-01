let
  configPath = ".github/dependabot.yml";
  automergeWorkflowPath = ".github/workflows/dependabot-automerge.yml";
in
{
  perSystem =
    { pkgs, ... }:
    {
      files.file = {
        ${configPath}.source = pkgs.writers.writeJSON "dependabot.yml" {
          version = 2;
          updates = [
            {
              package-ecosystem = "cargo";
              directory = "/";
              schedule.interval = "daily";
              commit-message = {
                prefix = "chore";
                include = "scope";
              };
            }
            {
              package-ecosystem = "nix";
              directory = "/";
              schedule.interval = "daily";
              commit-message.prefix = "chore";
            }
          ];
        };
        ${automergeWorkflowPath}.source = pkgs.writers.writeJSON "dependabot-automerge.yml" {
          name = "Dependabot auto-merge";
          on = "pull_request";

          permissions = {
            contents = "write";
            pull-requests = "write";
          };

          jobs.dependabot = {
            runs-on = "ubuntu-latest";
            "if" =
              "github.event.pull_request.user.login == 'dependabot[bot]' && github.repository == 'molybdenumsoftware/statix'";
            steps = [
              {
                name = "Dependabot metadata";
                id = "metadata";
                uses = "dependabot/fetch-metadata@main";
                "with".github-token = "\${{ secrets.GITHUB_TOKEN }}";

              }
              {

                name = "Enable auto-merge for Dependabot PRs";
                run = ''gh pr merge --auto --merge "$PR_URL"'';
                env = {

                  PR_URL = "\${{github.event.pull_request.html_url}}";
                  GH_TOKEN = "\${{secrets.GITHUB_TOKEN}}";
                };
              }
            ];
          };
        };
      };
      treefmt.settings.global.excludes = [
        configPath
        automergeWorkflowPath
      ];
    };
}
