# version-included

[![CI](https://github.com/lucasgomes-eti/version-included/actions/workflows/ci.yml/badge.svg)](https://github.com/lucasgomes-eti/version-included/actions/workflows/ci.yml) [![Quality gate status](https://sonarqube-lucasgomes-eti.up.railway.app/api/project_badges/measure?project=version-included&metric=alert_status&token=sqb_35e9db860736d5407b2c5b0c2f44f4d3d82ce33b)](https://sonarqube-lucasgomes-eti.up.railway.app/summary/new_code?id=version-included) [![Coverage](https://sonarqube-lucasgomes-eti.up.railway.app/api/project_badges/measure?project=version-included&metric=coverage&token=sqb_35e9db860736d5407b2c5b0c2f44f4d3d82ce33b)](https://sonarqube-lucasgomes-eti.up.railway.app/summary/new_code?id=version-included) [![Duplicated Lines (%)](https://sonarqube-lucasgomes-eti.up.railway.app/api/project_badges/measure?project=version-included&metric=duplicated_lines_density&token=sqb_35e9db860736d5407b2c5b0c2f44f4d3d82ce33b)](https://sonarqube-lucasgomes-eti.up.railway.app/summary/new_code?id=version-included) [![Lines of Code](https://sonarqube-lucasgomes-eti.up.railway.app/api/project_badges/measure?project=version-included&metric=ncloc&token=sqb_35e9db860736d5407b2c5b0c2f44f4d3d82ce33b)](https://sonarqube-lucasgomes-eti.up.railway.app/summary/new_code?id=version-included)

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

Install the latest release with the shell installer (Linux and macOS, x86_64 and ARM64):

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lucasgomes-eti/version-included/releases/latest/download/version-included-installer.sh | sh
```

The installer downloads the archive for your platform, verifies its checksum, installs `versinc`
to `~/.local/bin`, and adds that directory to `PATH` in your shell profile. Set
`VERSION_INCLUDED_INSTALL_DIR` to install somewhere else, or `VERSION_INCLUDED_NO_MODIFY_PATH=1`
to leave your shell profile alone. To pin a version, replace `latest/download` with
`download/vX.Y.Z`.

Prebuilt binaries are statically linked (musl) on Linux and are available for:

| Platform | Archive |
|---|---|
| Linux x86_64 | `version-included-x86_64-unknown-linux-musl.tar.xz` |
| Linux ARM64 | `version-included-aarch64-unknown-linux-musl.tar.xz` |
| macOS Intel | `version-included-x86_64-apple-darwin.tar.xz` |
| macOS Apple Silicon | `version-included-aarch64-apple-darwin.tar.xz` |

You can also download them from the [releases page](https://github.com/lucasgomes-eti/version-included/releases)
and check them against the `.sha256` files next to each archive. Windows is not supported.

To build from source instead (Rust 1.85 or newer):

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

| Commit                                                                               | Bump  |
| ------------------------------------------------------------------------------------ | ----- |
| `BREAKING CHANGE:` footer, or `!` after the type/scope (`feat!: …`, `feat(api)!: …`) | major |
| `feat`                                                                               | minor |
| `fix`, `perf`, `refactor`                                                            | patch |
| Any other type (`docs`, `style`, `test`, `build`, `chore`, …)                        | none  |
| A message that doesn't follow the header format                                      | none  |

- Types are matched exactly and case-sensitively against the header's type; the scope is ignored.
  `Feat: …` and `feature: …` bump nothing by default.
- Breaking-change markers are checked independently of the type: `docs!: …` is a major bump, and
  a `BREAKING CHANGE:` line in the body counts even if the header doesn't follow the format.
- Merge commits are always skipped, whatever their message says.

Examples, starting from tag `v1.2.3`:

| Commits since the tag          | Next version                                              |
| ------------------------------ | --------------------------------------------------------- |
| none                           | `1.2.3`                                                   |
| `fix: …`                       | `1.2.4`                                                   |
| `fix: …`, `feat: …`, `docs: …` | `1.3.0`                                                   |
| `feat(api)!: …`                | `2.0.0`                                                   |
| `docs: …`, `chore: …`          | `1.2.3` (or `1.2.5` with `bump_unmatched_commits = true`) |

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
(`1.4.0` → `1.4.1-dev.1`): a `1.4.0-dev.1` would sort _below_ the already released `1.4.0`.
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

| Option            | Effect                                               |
| ----------------- | ---------------------------------------------------- |
| `--with-prefix`   | Prefix printed versions with `tag_prefix` (`v1.5.0`) |
| `--output <file>` | Also write the version to a file                     |
| `--format json`   | Print details about the calculation as JSON          |
| `--config <path>` | Merge this config file over the others               |

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
    {
      "hash": "8b7e6d5…",
      "subject": "feat(search): add index",
      "bump": "minor"
    }
  ]
}
```

`previous_version`, `branch` and `prerelease_label` are `null` when there is no tag, no branch,
or no prerelease. With `breaking_bumps_minor_below_1`, a breaking commit is listed with
`"bump": "major"` while the top-level `bump` is `"minor"`.

### Exit codes and errors

| Code | Meaning                                                                            |
| ---- | ---------------------------------------------------------------------------------- |
| 0    | Success                                                                            |
| 1    | Error: not in a git repository, shallow clone, invalid config, unreadable files, … |
| 2    | Invalid command-line usage                                                         |

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
cargo llvm-cov              # tests with a coverage summary (needs cargo-llvm-cov)
cargo llvm-cov --lcov --output-path lcov.info   # LCOV report, as CI produces it
```

Install the coverage tool with `rustup component add llvm-tools` and
`cargo install cargo-llvm-cov --locked`.

The version logic is made of pure functions (`src/commit.rs`, `classify.rs`, `version.rs`,
`tags.rs`, `branch.rs`, `config.rs`, `output.rs`). Git, environment variables and the filesystem
sit behind the small `Git`, `Env` and `Fs` traits, which tests replace with fakes. Integration
tests in `tests/` run the real binary against temporary git repositories.

### Continuous integration

[`.github/workflows/ci.yml`](.github/workflows/ci.yml) runs on pushes to `main` and on pull
requests:

| Job                 | What it does                                                                                                                                                                                                                       |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Test (Linux, macOS) | `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and the test suite under `cargo llvm-cov`; the Linux job uploads `lcov.info`                                                                                     |
| Version             | Builds `versinc` from the same commit (`cargo run --release`) and calculates this repository's own version, shown as a notice and in the job summary. Pull requests get the branch's prerelease version through `GITHUB_HEAD_REF`. |
| SonarQube           | On pushes to `main` only, after the other jobs pass: analyzes the code with the LCOV coverage and Clippy findings, using the calculated version as the project version                                                             |

#### SonarQube setup

The analysis targets a self-hosted SonarQube Community Build, which analyzes a single branch, so
it only runs for `main`. Its built-in Rust analyzer runs Clippy itself during the scan and imports
coverage through `sonar.rust.lcov.reportPaths` (see
[`sonar-project.properties`](sonar-project.properties)).

1. Create a project on the server with the key `version-included` (or change `sonar.projectKey`).
2. Generate a token for it, and add two secrets to the `main` environment (Settings →
   Environments): `SONAR_TOKEN` (the token) and `SONAR_HOST_URL` (the server URL, e.g.
   `https://sonar.example.com`). The SonarQube job declares `environment: main`; environment
   secrets are not visible to jobs that don't.
3. The server must be reachable from GitHub-hosted runners.

### Releases

Releases are fully automated; nobody tags by hand. On every push to `main`, after the tests pass:

1. The **Version** job calculates the next version with `versinc` itself.
2. If it differs from the latest tag, the **Release** job (in `ci.yml`) uses a GitHub App
   installation token to commit the bumped version in `Cargo.toml` and `Cargo.lock` as
   `chore(release): vX.Y.Z`, create the tag `vX.Y.Z`, and push both atomically. If `main` moved
   in the meantime, nothing is pushed and the next run releases instead.
3. The tag push starts [`release.yml`](.github/workflows/release.yml), generated by
   [dist](https://github.com/axodotdev/cargo-dist), which builds the four targets natively,
   generates the shell installer and checksums, and publishes a GitHub Release.

The App token matters: pushes made with the default `GITHUB_TOKEN` don't start other workflows,
so a tag pushed with it would never trigger `release.yml`. The release commit is a `chore`, so the
CI run it starts calculates the same version and releases nothing.

The version a merge produces depends on its commits, so with **squash merging** the pull request
title must follow the commit convention (e.g. `feat(release): …`); with merge commits or rebase
merging, the branch's own commits count and the merge commit itself is ignored.

`release.yml` and `dist-workspace.toml` are managed by dist: change `dist-workspace.toml` and run
`dist generate` instead of editing `release.yml`, otherwise dist's check on pull requests fails.
Install dist with `cargo install --git https://github.com/axodotdev/cargo-dist --tag v0.33.1
cargo-dist --locked` (the version pinned as `cargo-dist-version`).

#### Release bot setup

The Release job needs a GitHub App that can push to this repository:

1. Create a GitHub App (Settings → Developer settings → GitHub Apps → New GitHub App) with
   webhooks disabled and the single repository permission **Contents: Read and write**.
2. Install it on this repository only.
3. In the `main` environment, add the App's Client ID as the variable `RELEASE_APP_CLIENT_ID` and
   a generated private key as the secret `RELEASE_APP_PRIVATE_KEY`.

Restrict the `main` environment to the `main` branch (Deployment branches and tags → Selected
branches) so only workflows running on `main` can use the App's key.

## License

[Apache-2.0](LICENSE)
