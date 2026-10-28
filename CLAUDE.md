# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`version-included` is a Rust CLI tool that generates a version number from commit messages. It's in the early scaffold stage: just a Cargo binary crate (`src/main.rs`), Rust edition 2024, with no dependencies yet. Licensed under Apache-2.0.

## Commands

```sh
cargo build                 # build
cargo run -- <args>         # run the CLI
cargo test                  # run all tests
cargo test <name>           # run tests whose name matches <name>
cargo fmt                   # format
cargo clippy --all-targets  # lint
```

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
- docs for changes to the documentation.
- style for formatting changes, missing semicolons, etc.
- refactor for refactoring production code, e.g. renaming a variable.
- test for adding missing tests, refactoring tests; no production code change.
- build for updating build configuration, development tools or other changes irrelevant to the user.

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
