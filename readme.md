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
```

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
ineffective_string_escape
negated_is_null
optionals_string
optional_list_in_flat_context
```

All lints are enabled by default. Generate a minimal config
with `statix dump > statix.toml`.

`ineffective_string_escape` removes only inert backslash-space escapes in ordinary
quoted strings. It preserves escaped backslashes, valid escapes, and indented
strings; see the [Home Manager correction](https://github.com/nix-community/home-manager/pull/8916).

Builtin diagnostics respect lexical shadowing of `builtins`, `toPath`, `isNull`,
and `null`. `negated_is_null` is a configurable readability hint, not a deprecation:
`isNull` remains a supported builtin. The fix preserves expression grouping and
leaves higher-order predicates alone; disable the hint if your project prefers
the predicate spelling. Comments outside the argument prevent automatic replacement.

Nixpkgs helper checks assume a conventional `lib` input and skip local library
definitions. `optionals_string` recommends `optionalString` only for a literal
string concatenation. `optional_list_in_flat_context` recommends `optionals`
only for a literal list flowing directly or through `++` into `lib.makeBinPath`.
Both replace only the helper name, retaining arguments and comments. Unresolved
aliases, ordinary nested lists, and unknown consumers are not rewritten.

## Maintainer coverage benchmark

Run `bash autoresearch.sh` to measure detection and fix coverage against
`bin/benchmarks/maintainer_cases.json`. The corpus contains adapted, cited
examples from Nixpkgs, Home Manager, Nix, RFCs, and NixOS Discourse, plus
existing-rule controls and valid near-misses. It is a curated sample, not
a measurement of issue frequency.

Provision the existing development environment and dependency cache once:

```shell
nix develop --command cargo fetch --locked
bash autoresearch.sh
```

Benchmark runs use locked, offline builds and embedded fixtures; no fetching
or Nix evaluation occurs in the workload. The driver exercises the production
lint dispatcher and fix iterator with all default rules. `METRIC maintainer_f1`
is detection F1, expressed as a percentage; unrelated warnings do not count
as detecting a case. Accepted examples and near-misses penalize false positives.
Existing-rule controls are reported separately from researched positives.

Secondary metrics report exact token-level fix coverage (ignoring whitespace),
false positives, unexpected changes, and control coverage. Fixtures distinguish
automatic-fix candidates from advisory cases; their proposed rule names define
the diagnostic contract for future implementations, not currently available lints.
Invalid fixtures, invalid generated syntax, or non-converging fixes fail the run.
Missing diagnostics remain measured gaps. Matching tokens does not establish
semantic equivalence; module, scope, and shell corrections require the contextual
checks recorded in each case's rationale.
