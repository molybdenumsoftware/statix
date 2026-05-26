{ lib, config, ... }:
{
  options.gitignore = lib.mkOption {
    type = lib.types.listOf lib.types.singleLineStr;
    apply = lib.flip lib.pipe [
      lib.naturalSort
      lib.concatLines
    ];
  };
  config = {
    gitignore = [ "/result" ];

    perSystem = {
      files.file.".gitignore".text = config.gitignore;
    };
  };
}
