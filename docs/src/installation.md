# Installation

`versinc` needs the `git` executable on `PATH`.

## Shell installer

Install the latest release with the shell installer (Linux x86_64 and ARM64, macOS Apple Silicon):

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/lucasgomes-eti/version-included/releases/latest/download/version-included-installer.sh | sh
```

The installer downloads the archive for your platform, verifies its checksum, installs `versinc`
to `~/.local/bin`, and adds that directory to `PATH` in your shell profile. Set
`VERSION_INCLUDED_INSTALL_DIR` to install somewhere else, or `VERSION_INCLUDED_NO_MODIFY_PATH=1`
to leave your shell profile alone. To pin a version, replace `latest/download` with
`download/vX.Y.Z`.

## Prebuilt binaries

Prebuilt binaries are statically linked (musl) on Linux and are available for:

| Platform            | Archive                                              |
| ------------------- | ---------------------------------------------------- |
| Linux x86_64        | `version-included-x86_64-unknown-linux-musl.tar.xz`  |
| Linux ARM64         | `version-included-aarch64-unknown-linux-musl.tar.xz` |
| macOS Apple Silicon | `version-included-aarch64-apple-darwin.tar.xz`       |

You can also download them from the [releases page](https://github.com/lucasgomes-eti/version-included/releases)
and check them against the `.sha256` files next to each archive. Windows is not supported, and
releases after v0.3.0 have no Intel macOS binary; on an Intel Mac, build from source.

## Building from source

To build from source instead (Rust 1.85 or newer):

```sh
cargo install --git https://github.com/lucasgomes-eti/version-included
```

## Updating

```sh
versinc update --check   # report whether a newer release exists
versinc update           # install it
```

`versinc update` downloads the latest release archive for your platform, verifies its sha256
checksum, and replaces the running executable in place, wherever it is installed. It never runs in
the background and never checks on its own. It uses `curl`, `tar` (with xz support) and
`sha256sum` or `shasum`, which Linux and macOS provide. If the executable's directory isn't
writable (for example `/usr/local/bin`), it stops with an error instead of asking for elevated
permissions.

In CI there is nothing to update: the [GitHub Action](github-action.md) installs a release on every
run (`version: latest` always installs the newest one).

`VERSINC_RELEASES_URL` points `update` at a mirror of the releases instead of GitHub (`https://` or
`file://`). The mirror needs the GitHub Releases layout: `latest/download/dist-manifest.json` and
`download/vX.Y.Z/<archive>`.
