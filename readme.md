# statix

> Lints and suggestions for the Nix programming language.

Reluctantly forked from [oppiliappan/statix](https://github.com/oppiliappan/statix)
because I wanted to maintain and could not obtain some repository permission bits.
I still hope to unfork.

`statix check` highlights antipatterns in Nix code. `statix
fix` can fix several such occurrences.

For the time-being, `statix` works only with ASTs
produced by the `rnix-parser` crate and does not evaluate
any nix code (imports, attr sets etc.).

## Examples

```shell
$ statix check tests/c.nix
[manual_inherit_from] Warning: Assignment instead of inherit from
   ╭─[tests/c.nix:2:3]
   │
 2 │   mtl = pkgs.haskellPackages.mtl;
   ·   ───────────────┬───────────────
   ·                  ╰───────────────── This assignment is better written with inherit
───╯

$ statix fix --dry-run tests/c.nix
--- tests/c.nix
+++ tests/c.nix [fixed]
@@ -1,6 +1,6 @@
 let
-  mtl = pkgs.haskellPackages.mtl;
+  inherit (pkgs.haskellPackages) mtl;
 in
 null
```

## Installation

`statix` is available via a nix flake:

```shell
# build from source
nix build git+https://git.peppe.rs/languages/statix
./result/bin/statix --help

# statix also provides a flake app
nix run git+https://git.peppe.rs/languages/statix -- --help

# save time on builds using cachix
cachix use statix
```

Install from nixpkgs:

```shell
nix run nixpkgs#statix -- help
```

Install with [brew/linuxbrew](https://brew.sh)

```bash
brew install statix
```

## No releases

Even though some releases were made,
no more are intended.
Instead, this project is meant to be used from HEAD.

## Usage

Basic usage is as simple as:

```shell
# recursively finds nix files and raises lints
statix check /path/to/dir

# ignore generated files, such as Cargo.nix
statix check /path/to/dir -i Cargo.nix

# ignore more than one file
statix check /path/to/dir -i a.nix b.nix c.nix

# ignore an entire directory
statix check /path/to/dir -i .direnv

# statix respects your .gitignore if it exists
# run statix in "unrestricted" mode, to disable that
statix check /path/to/dir -u

# see `statix -h` for a full list of options
```

Certain lints have suggestions. Apply suggestions back to
the source with:

```shell
statix fix /path/to/file

# show diff, do not write to file
statix fix --dry-run /path/to/file
```

`statix` supports a variety of output formats; standard,
json and errfmt:

```shell
statix check /path/to/dir -o json
statix check /path/to/dir -o errfmt # singleline, easy to integrate with vim
statix check /path/to/dir -o agent  # markdown with context and fix instructions, for coding agents
```

### Scripts in and next to Nix code

Three lints check scripts with [ShellCheck](https://www.shellcheck.net)
and [ruff](https://docs.astral.sh/ruff) (install them, or point
`STATIX_SHELLCHECK` / `STATIX_RUFF` at them; without them the lints do
nothing):

- `shellcheck`: shell scripts written as Nix strings: devenv `scripts`,
  `tasks`, `processes`, `enterShell`, `enterTest`; nixpkgs
  `writeShellScript`, `writeShellApplication`, `writers.writeBash` and
  friends, `runCommand`, stdenv phases and hooks (`buildPhase`,
  `postInstall`, `shellHook`...); NixOS `systemd.services.*.script`,
  `preStart`... and `system.activationScripts`; and `let` bindings used
  as or interpolated into those
- `ruff`: Python scripts written as Nix strings: `writers.writePython3`,
  `writeScript` with a Python shebang, devenv scripts with a Python
  `package`
- `script_file`: `.sh`, `.bash` and `.py` files (or files with such a
  shebang) that Nix code refers to: `./deploy.sh`, `builtins.readFile
./x.sh`, `"${./x.sh}"`, `updateScript = ./update.sh`, setup hooks, and
  files devenv scripts run by relative path (`python scripts/x.py`)

Findings are reported at their position in the `.nix` file (for
`script_file`, at the reference, with the script's own line and column).

`statix fix` applies the checkers' fixes: ShellCheck's own plus built-in
ones for SC2045, SC2115, SC2155 and SC2162, and ruff's safe fixes. In Nix
strings they are escaped for `''...''` and `"..."` strings and leave `${...}`
interpolations alone; every fix is re-parsed and checked to render exactly
the intended script before it's applied. Referenced script files are fixed
in place (`--dry-run` shows the diff).

Everything else needs a decision. `-o agent` lists it with the
surrounding source, a hint for common codes, a link to the rule's docs
and the Nix escaping rules, so a coding agent can finish the job:

```shell
statix fix . && statix check -o agent .
```

In `-o json` every diagnostic has `fixable` (whether `statix fix`
handles it), `help` when it doesn't, and `external` (file, line, column)
for findings in referenced scripts.

### Configuration

Ignore lints and fixes by creating a `statix.toml` file at
your project root:

```
# within statix.toml
disabled = [
  "empty_pattern"
]
```

`statix` automatically discovers the configuration file by
traversing parents of the current directory and looking for
a `statix.toml` file. Alternatively, you can pass the path
to the `statix.toml` file on the command line with the
`--config` flag (available on `statix check` and `statix
fix`).

The available lints are (see `statix list` for an updated
list):

```
bool_comparison
empty_let_in
manual_inherit
manual_inherit_from
legacy_let_syntax
collapsible_let_in
eta_reduction
useless_parens
empty_pattern
redundant_pattern_bind
unquoted_uri
empty_inherit
deprecated_to_path
bool_simplification
useless_has_attr
repeated_keys
empty_list_concat
devenv_exec_shebang
devenv_pre_commit
hardcoded_store_path
impure_host_path
shellcheck
ruff
script_file
```

All lints are enabled by default. Generate a minimal config
with `statix dump > statix.toml`.
