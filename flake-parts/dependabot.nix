let
  filePath = ".github/dependabot.yml";
in
{
  perSystem =
    { pkgs, ... }:
    {
      files.file.${filePath}.source = pkgs.writers.writeJSON "dependabot.yml" {
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
            schedule.interval = "daily";
            commit-message.prefix = "chore";
          }
        ];
      };
      treefmt.settings.global.excludes = [ filePath ];
    };
}
