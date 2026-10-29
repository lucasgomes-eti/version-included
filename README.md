# version-included

`versinc` calculates the next [SemVer 2.0.0](https://semver.org/) version of a project from its git
history: it finds the latest release tag, reads the commit messages since then, and prints the
version that release should get.

```console
$ git tag
v1.4.0
$ git log --oneline v1.4.0..HEAD
3f2c1a9 docs: describe search
8b7e6d5 feat(search): add index
$ versinc
1.5.0
```

`versinc` only reads the repository. It never creates tags or commits.

## Installation

Prebuilt binaries are not published yet. Build from source with Rust 1.85 or newer:

```sh
cargo install --git https://github.com/lucasgomes-eti/version-included
```

`versinc` needs the `git` executable on `PATH`.

## How the version is calculated

1. **Find the base version.** Among the tags reachable from `HEAD`, take the highest version that
   parses as SemVer after stripping the tag prefix (default `v`). Tags that don't parse
   (`release-5`, `v2`, `nightly`) and prerelease tags (`v2.0.0-rc.1`) are ignored. When several
   tags point at the same commit, the highest wins.
2. **Read the commits since that tag**, skipping merge commits.
3. **Classify each commit** and apply the highest bump found (major > minor > patch > none).
4. **Apply prerelease rules** based on the current branch.
5. **Print the version.**

If there is no release tag yet, `versinc` prints the configured initial version (default `0.1.0`)
as is, without bumping it; that is the version of your first release.

### Commit rules

Commit headers follow the [Karma convention](https://karma-runner.github.io/6.4/dev/git-commit-msg.html),
`<type>(<scope>): <subject>`:

| Commit | Bump |
|---|---|
| `BREAKING CHANGE:` footer, or `!` after the type/scope (`feat!: …`, `feat(api)!: …`) | major |
| `feat` | minor |
| `fix`, `perf`, `refactor` | patch |
| Any other type (`docs`, `style`, `test`, `build`, `chore`, …) | none |
| A message that doesn't follow the header format | none |

- Types are matched exactly and case-sensitively against the header's type; the scope is ignored.
  `Feat: …` and `feature: …` bump nothing by default.
- Breaking-change markers are checked independently of the type: `docs!: …` is a major bump, and
  a `BREAKING CHANGE:` line in the body counts even if the header doesn't follow the format.
- Merge commits are always skipped, whatever their message says.

Examples, starting from tag `v1.2.3`:

| Commits since the tag | Next version |
|---|---|
| none | `1.2.3` |
| `fix: …` | `1.2.4` |
| `fix: …`, `feat: …`, `docs: …` | `1.3.0` |
| `feat(api)!: …` | `2.0.0` |
| `docs: …`, `chore: …` | `1.2.3` (or `1.2.5` with `bump_unmatched_commits = true`) |

### Edge cases

- **Only non-matching commits.** By default the version stays the same. With
  `bump_unmatched_commits = true`, the patch is bumped by the number of commits since the tag
  (`1.2.3` + 2 commits → `1.2.5`). As soon as one commit matches a rule, the normal rules apply
  and this option is ignored.
- **Breaking changes below 1.0.0.** By default they bump the major (`0.4.2` → `1.0.0`). With
  `breaking_bumps_minor_below_1 = true` they bump the minor instead while the major is 0
  (`0.4.2` → `0.5.0`).

### Branches and prereleases

When the current branch matches a prerelease pattern (default `feature/*` → `dev`) and there are
commits since the tag, `versinc` appends `-<label>.<number of commits since the tag>`:

```console
$ git switch -c feature/search    # from v1.4.0
$ git commit -m "feat(search): add index"
$ git commit -m "docs: describe search"
$ versinc
1.5.0-dev.2
```

If the commits on the branch don't bump anything, the patch is bumped before adding the label
(`1.4.0` → `1.4.1-dev.1`): a `1.4.0-dev.1` would sort *below* the already released `1.4.0`.
A branch with no commits since the tag gets the plain release version.

The branch comes from `git rev-parse --abbrev-ref HEAD`. When `HEAD` is detached, as it often is
in CI, `versinc` uses the `GITHUB_HEAD_REF` environment variable instead (set by GitHub Actions on
pull requests). If neither names a branch, the build is treated as a release build and a warning
is printed to stderr.

## Configuration

`versinc` works without any configuration. To change the defaults, write a TOML file:

```sh
versinc init            # writes versinc.toml at the repository root
versinc init --global   # writes ~/.config/version-included/config.toml
```

Both write the fully commented defaults ([`config/default.toml`](config/default.toml)) and refuse
to overwrite an existing file unless you pass `--force`. `init --global` creates the directory if
needed.

### Layering

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

### Reference

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

Config files are validated when loaded. Unknown keys, wrong types and invalid values are errors
that name the file and the key:

```console
$ versinc
error: invalid config file /work/app/versinc.toml: key `initial_version`: "1.2" is not a valid SemVer version (unexpected end of input while parsing minor version number)
```

## Output

By default `versinc` prints only the version to stdout, without the tag prefix. Everything else
(warnings, errors) goes to stderr, so `VERSION=$(versinc)` is safe in scripts.

| Option | Effect |
|---|---|
| `--with-prefix` | Prefix printed versions with `tag_prefix` (`v1.5.0`) |
| `--output <file>` | Also write the version to a file |
| `--format json` | Print details about the calculation as JSON |
| `--config <path>` | Merge this config file over the others |

`--format json` prints the previous and next version, the bump applied to the version core, the
branch, the prerelease label, the config files that were loaded, and the commits that caused the
bump:

```json
{
  "previous_version": "1.4.0",
  "next_version": "1.5.0-dev.2",
  "bump": "minor",
  "branch": "feature/search",
  "prerelease_label": "dev",
  "config_files": ["/work/app/versinc.toml"],
  "commits": [
    { "hash": "8b7e6d5…", "subject": "feat(search): add index", "bump": "minor" }
  ]
}
```

`previous_version`, `branch` and `prerelease_label` are `null` when there is no tag, no branch,
or no prerelease. With `breaking_bumps_minor_below_1`, a breaking commit is listed with
`"bump": "major"` while the top-level `bump` is `"minor"`.

### Exit codes and errors

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Error: not in a git repository, shallow clone, invalid config, unreadable files, … |
| 2 | Invalid command-line usage |

`versinc` refuses to run in a **shallow clone**, because missing history and tags would produce a
wrong version. Fetch the full history first (`git fetch --unshallow --tags`); with
`actions/checkout`, set `fetch-depth: 0`.

## Development

```sh
cargo build                 # build
cargo run -- <args>         # run versinc
cargo test                  # unit and integration tests
cargo fmt                   # format
cargo clippy --all-targets  # lint
```

The version logic is made of pure functions (`src/commit.rs`, `classify.rs`, `version.rs`,
`tags.rs`, `branch.rs`, `config.rs`, `output.rs`). Git, environment variables and the filesystem
sit behind the small `Git`, `Env` and `Fs` traits, which tests replace with fakes. Integration
tests in `tests/` run the real binary against temporary git repositories.

## License

[Apache-2.0](LICENSE)
