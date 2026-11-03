# Development

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

## Documentation site

The project site is built with [oranda](https://github.com/axodotdev/oranda) from
[`oranda.json`](https://github.com/lucasgomes-eti/version-included/blob/main/oranda.json): the
README becomes the home page, this book (`docs/`, an [mdbook](https://rust-lang.github.io/mdBook/))
is published under `book/`, and the install page lists the latest release's artifacts from its
`dist-manifest.json`. To preview it:

```sh
cargo install oranda --locked --version 0.6.5   # once
oranda dev   # serves http://127.0.0.1:7979/version-included/ and rebuilds on changes
```

`oranda build` writes the site to `public/` without serving it. Images the README uses must live
in `static/`, which oranda copies into the site as is, so the same relative path works on GitHub
and on the site. Colors and the header live in `docs/css/versinc.css`, which both `oranda.json`
and `docs/book.toml` load.

The README is the site's home page, but oranda doesn't rewrite its relative links, so the README
links to the articles by their published URLs. The book's own pages link to each other with
relative `.md` links. See [Site publishing](releasing.md#site-publishing) for how the site is
deployed.

## Architecture

The version logic is made of pure functions (`src/commit.rs`, `classify.rs`, `version.rs`,
`tags.rs`, `branch.rs`, `config.rs`, `output.rs`). Git, environment variables and the filesystem
sit behind the small `Git`, `Env` and `Fs` traits, which tests replace with fakes. Integration
tests in `tests/` run the real binary against temporary git repositories.

## Continuous integration

[`.github/workflows/ci.yml`](https://github.com/lucasgomes-eti/version-included/blob/main/.github/workflows/ci.yml)
runs on pushes to `main` and on pull requests:

| Job       | What it does                                                                                                                                                                                                                       |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Test      | On `ubuntu-latest`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and the test suite under `cargo llvm-cov`, then uploads `lcov.info`                                                                          |
| Version   | Builds `versinc` from the same commit (`cargo run --release`) and calculates this repository's own version, shown as a notice and in the job summary. Pull requests get the branch's prerelease version through `GITHUB_HEAD_REF`. |
| SonarQube | On pushes to `main` only, after the other jobs pass: analyzes the code with the LCOV coverage and Clippy findings, using the calculated version as the project version                                                             |
| Release   | On pushes to `main` only, when the calculated version changed: commits and tags the release (see [Releases](releasing.md))                                                                                                         |

[`.github/workflows/action-test.yml`](https://github.com/lucasgomes-eti/version-included/blob/main/.github/workflows/action-test.yml)
tests the public action on `ubuntu-latest` (defaults, `args`, `latest`, binary on `PATH`) and
checks that it fails on Windows and on shallow clones. It runs on pull requests and pushes, and
dist calls it from `release.yml` after every release (`post-announce-jobs`), so each release's
action is tested against its own published binaries.

### SonarQube setup

The analysis targets a self-hosted SonarQube Community Build, which analyzes a single branch, so
it only runs for `main`. Its built-in Rust analyzer runs Clippy itself during the scan and imports
coverage through `sonar.rust.lcov.reportPaths` (see
[`sonar-project.properties`](https://github.com/lucasgomes-eti/version-included/blob/main/sonar-project.properties)).

1. Create a project on the server with the key `version-included` (or change `sonar.projectKey`).
2. Generate a token for it, and add two secrets to the `main` environment (Settings →
   Environments): `SONAR_TOKEN` (the token) and `SONAR_HOST_URL` (the server URL, e.g.
   `https://sonar.example.com`). The SonarQube job declares `environment: main`; environment
   secrets are not visible to jobs that don't.
3. The server must be reachable from GitHub-hosted runners.
