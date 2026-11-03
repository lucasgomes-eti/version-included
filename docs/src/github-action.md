# GitHub Action

This repository is also a GitHub Action that installs `versinc` and calculates the version in
your workflows:

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

> **Important:** `actions/checkout` makes a shallow clone by default (`fetch-depth: 1`), with no
> tags and only one commit. Always set **`fetch-depth: 0`**; on a shallow clone `versinc` fails
> rather than calculate a wrong version.

## Inputs and outputs

| Input     | Default                               | Description                                                                                                                    |
| --------- | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `version` | the release matching the action's tag | `versinc` release to install, e.g. `0.3.0` or `latest`                                                                         |
| `args`    | `""`                                  | Extra arguments, split on whitespace (no shell quoting), e.g. `--with-prefix --config ci/versinc.toml`. Don't pass `--output`. |

| Output    | Description                                                          |
| --------- | -------------------------------------------------------------------- |
| `version` | The calculated version, e.g. `1.5.0` (`v1.5.0` with `--with-prefix`) |

See [Output and exit codes](output.md) for the options `args` accepts.

## How it runs

The action downloads the release archive for the runner from this repository's GitHub Releases,
verifies its sha256 checksum, and adds `versinc` to `PATH`, so later steps can run it directly
(for example `versinc --format json`). It runs on Linux runners (X64 and ARM64) and macOS ARM64
runners; on Windows and Intel macOS runners it fails with an explicit error.

On pull requests, GitHub checks out a detached merge commit; `versinc` then takes the branch
name from `GITHUB_HEAD_REF`, so `feature/*` pull requests get prerelease versions as expected
(see [Branches and prereleases](how-it-works.md#branches-and-prereleases)).
