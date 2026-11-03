# Configuration

`versinc` works without any configuration. To change the defaults, write a TOML file:

```sh
versinc init            # writes versinc.toml at the repository root
versinc init --global   # writes ~/.config/version-included/config.toml
```

Both write the fully commented defaults
([`config/default.toml`](https://github.com/lucasgomes-eti/version-included/blob/main/config/default.toml))
and refuse to overwrite an existing file unless you pass `--force`. `init --global` creates the
directory if needed.

## Layering

Configuration is merged key by key from these sources, later ones winning:

1. Built-in defaults (compiled into the binary)
2. Global config: `$XDG_CONFIG_HOME/version-included/config.toml`, or
   `~/.config/version-included/config.toml` when `XDG_CONFIG_HOME` is unset (on macOS too)
3. Project config: `versinc.toml` at the git repository root (found even when you run `versinc`
   from a subdirectory)
4. The file passed with `--config <path>`

Missing global and project files are skipped; a missing `--config` file is an error. Each file
only needs the keys it changes.

**Lists replace, they don't append.** If the defaults say `patch = ["fix", "perf", "refactor"]`
and your project config says `patch = ["fix"]`, the patch list is just `["fix"]`. The same goes
for `[[prerelease]]`: defining any prerelease rule in a file replaces all inherited rules. Tables
are merged key by key, so setting `[rules] patch` keeps the inherited `minor` and `major` lists.

## Reference

```toml
# Prefix stripped from tag names before parsing them as SemVer ("" for unprefixed tags).
tag_prefix = "v"

# Version printed when no release tag exists yet. Must be a plain MAJOR.MINOR.PATCH version.
initial_version = "0.1.0"

# Bump the patch by the number of commits when none of them matches a rule.
bump_unmatched_commits = false

# While the major version is 0, breaking changes bump the minor instead of the major.
breaking_bumps_minor_below_1 = false

# Commit types for each bump level. Breaking-change markers always mean major.
[rules]
major = []
minor = ["feat"]
patch = ["fix", "perf", "refactor"]

# Prerelease labels by branch pattern, checked in order; the first match wins.
# `*` matches any characters, including `/`; `?` matches one character.
# Labels must be valid SemVer prerelease identifiers. `prerelease = []` disables prereleases.
[[prerelease]]
branch = "feature/*"
label = "dev"

[[prerelease]]
branch = "release/*"
label = "rc"
```

See [How the version is calculated](how-it-works.md) for what each option does.

## Validation

Config files are validated when loaded. Unknown keys, wrong types and invalid values are errors
that name the file and the key:

```console
$ versinc
error: invalid config file /work/app/versinc.toml: key `initial_version`: "1.2" is not a valid SemVer version (unexpected end of input while parsing minor version number)
```
