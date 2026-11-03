![version-included](static/readme_header.png)

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

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lucasgomes-eti/version-included/releases/latest/download/version-included-installer.sh | sh
```

For the supported platforms, installer options and building from source, see
[Installation](docs/src/installation.md).

## Usage

Run `versinc` anywhere in a git repository. It works without any configuration; see
[How the version is calculated](docs/src/how-it-works.md) for the default rules.

```sh
versinc                 # print the next version
versinc init            # write the commented defaults to versinc.toml at the repository root
versinc init --global   # or to ~/.config/version-included/config.toml
versinc update --check  # report whether a newer release exists
versinc update          # install it
```

See [Configuration](docs/src/configuration.md) and [Updating](docs/src/installation.md#updating)
for details.

## GitHub Action

```yaml
jobs:
  version:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0 # required: versinc needs the full history and tags

      - id: versinc
        uses: lucasgomes-eti/version-included@v0.4.1 # pin a release tag

      - run: echo "Building version ${{ steps.versinc.outputs.version }}"
```

> **Important:** always set `fetch-depth: 0`. On the shallow clone `actions/checkout` makes by
> default, `versinc` fails rather than calculate a wrong version.

See [GitHub Action](docs/src/github-action.md) for its inputs, outputs and supported runners.

## Documentation

- [Installation](docs/src/installation.md): platforms, installer options, building from source,
  updating
- [How the version is calculated](docs/src/how-it-works.md): commit rules, edge cases, branches
  and prereleases
- [Configuration](docs/src/configuration.md): config files, layering, reference
- [Output and exit codes](docs/src/output.md): options, JSON output, errors
- [GitHub Action](docs/src/github-action.md): inputs, outputs, supported runners
- [Development](docs/src/development.md) and [Releases](docs/src/releasing.md): building, testing,
  CI and the release pipeline

## License

[Apache-2.0](LICENSE)
