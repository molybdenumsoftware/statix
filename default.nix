# The `default.nix` in flake-compat reads `flake.nix` and `flake.lock` from `src` and
# returns an attribute set of the shape `{ defaultNix, shellNix }`

let
  lock = builtins.fromJSON (builtins.readFile ./flake.lock);
  inherit (lock.nodes.flake-compat.locked)
    owner
    repo
    rev
    narHash
    ;
  flake-compat = fetchTarball {
    url = "https://github.com/${owner}/${repo}/archive/${rev}.tar.gz";
    sha256 = narHash;
  };
in
(import flake-compat { src = ./.; }).defaultNix
