# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`version-included` is a Rust CLI (binary `versinc`) that calculates the next SemVer version from git
history. It is read-only: it never creates tags or commits. Rust edition 2024, Apache-2.0. The
README documents user-facing behavior; keep it in sync as features land.

## Architecture

The crate is a library (`src/lib.rs`) plus a thin binary (`src/main.rs`).

- Pure logic, no git or filesystem access: `commit.rs` (header/footer parsing), `classify.rs`
  (commit → `Bump`), `version.rs` (`calculate`), `tags.rs` (latest SemVer tag), `branch.rs`
  (glob matching, branch resolution), `config.rs` (layer parsing, validation, merging),
  `output.rs` (text/JSON rendering).
- Boundaries: `git.rs` (`Git` trait, `SystemGit` shells out to `git`), `env.rs` (`Env` and `Fs`
  traits), `update.rs` (`Tools` trait, `SystemTools` shells out to `curl`, `sha256sum`/`shasum` and
  `tar`). Unit tests use the fakes in `env::tests`, `update::tests` and `app::tests`.
- `update.rs` implements `versinc update`: reads the latest version from the release's
  `dist-manifest.json`, verifies the archive checksum, unpacks into `.versinc-update/` next to the
  binary and renames over it. Integration tests use a `file://` mirror via `VERSINC_RELEASES_URL`.
- `app.rs` orchestrates a run given those boundaries; `cli.rs` holds the clap definitions.
- `config/default.toml` is embedded with `include_str!` as the built-in defaults and is what
  `versinc init` writes; a test asserts they are identical.
- Integration tests (`tests/cli.rs`, helpers in `tests/support/`) run the binary against temporary
  git repos isolated from the user's git config and environment.

## Commands

```sh
cargo build                 # build
cargo run -- <args>         # run the CLI
cargo test                  # run all tests
cargo test <name>           # run tests whose name matches <name>
cargo fmt                   # format
cargo clippy --all-targets  # lint
cargo llvm-cov              # tests with coverage summary (needs cargo-llvm-cov + llvm-tools)
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy with `-D warnings`, and the tests under
`cargo llvm-cov` on Linux and macOS, prints this repo's own version with `versinc` built from the
same commit, and runs SonarQube analysis on pushes to `main` only. When the calculated version
changes on `main`, its Release job has a GitHub App bot push `chore(release): vX.Y.Z` plus the tag,
which starts dist's `release.yml`. `release.yml` is generated: edit `dist-workspace.toml` and run
`dist generate` instead of editing it.

The public GitHub Action (`action.yml`, composite) is a separate concern from this repo's own
workflows. It installs a released binary; its `version` input default is marked
`# x-release-version` and rewritten by the Release job in each `chore(release)` commit.
`action-test.yml` tests it on PRs and pushes, and dist calls it after each release via
`post-announce-jobs`.

Edition 2024 requires Rust 1.85 or newer.

## Git Commit Message

In the repository we use and enforce the commit message conventions.

### Format of the commit message:

```
<type>(<scope>): <subject>
<BLANK LINE>
<body>
<BLANK LINE>
<footer>
```

### Example commit message:

```
fix(middleware): ensure Range headers adhere more closely to RFC 2616

Add one new dependency, use `range-parser` (Express dependency) to compute
range. It is more well-tested in the wild.

Fixes #2310
```

### Message subject (first line)

The first line cannot be longer than 72 characters and should be followed by a blank line. The type
and scope should always be lowercase as shown below.

#### Allowed <type> values:

- feat for a new feature for the user, not a new feature for build script. Such commit will trigger
  a release bumping a MINOR version.
- fix for a bug fix for the user, not a fix to a build script. Such commit will trigger a release
  bumping a PATCH version.
- perf for performance improvements. Such commit will trigger a release bumping a PATCH version.
- refactor for refactoring production code, e.g. renaming a variable. Such commit will trigger a
  release bumping a PATCH version.
- docs for changes to the documentation.
- style for formatting changes, missing semicolons, etc.
- test for adding missing tests, refactoring tests; no production code change.
- build for updating build configuration, development tools or other changes irrelevant to the user.
- chore for maintenance that fits no other type, such as the automated `chore(release): vX.Y.Z`
  version bump commit. Such commit does not trigger a release.

#### Example <scope> values:

- init
- runner
- watcher
- config
- web-server
- proxy
- etc.

The <scope> can be empty (e.g. if the change is a global or difficult to assign to a single
component), in which case the parentheses are omitted.

### Message body

Just as in the <subject>, use the imperative, present tense: "change" not "changed" nor "changes".
Message body should include motivation for the change and contrasts with previous behavior.

### Message footer

#### Referencing issues

Closed issues should be listed on a separate line in the footer prefixed with "Closes" keyword like
this:

`Closes #234`

or in the case of multiple issues:

`Closes #123, #245, #992`

#### Breaking changes

All breaking changes have to be mentioned in footer with the description of the change,
justification and migration notes.

```
BREAKING CHANGE:

`port-runner` command line option has changed to `runner-port`, so that it is
consistent with the configuration file syntax.

To migrate your project, change all the commands, where you use `--port-runner`
to `--runner-port`.
```

Any commit with the breaking change section will trigger a MAJOR release and appear on the changelog
independently of the commit type.
