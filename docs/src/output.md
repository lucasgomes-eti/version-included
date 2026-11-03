# Output and exit codes

By default `versinc` prints only the version to stdout, without the tag prefix. Everything else
(warnings, errors) goes to stderr, so `VERSION=$(versinc)` is safe in scripts.

| Option            | Effect                                               |
| ----------------- | ---------------------------------------------------- |
| `--with-prefix`   | Prefix printed versions with `tag_prefix` (`v1.5.0`) |
| `--output <file>` | Also write the version to a file                     |
| `--format json`   | Print details about the calculation as JSON          |
| `--config <path>` | Merge this config file over the others               |

## JSON output

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

## Exit codes and errors

| Code | Meaning                                                                            |
| ---- | ---------------------------------------------------------------------------------- |
| 0    | Success                                                                            |
| 1    | Error: not in a git repository, shallow clone, invalid config, unreadable files, … |
| 2    | Invalid command-line usage                                                         |

`versinc` refuses to run in a **shallow clone**, because missing history and tags would produce a
wrong version. Fetch the full history first (`git fetch --unshallow --tags`); with
`actions/checkout`, set `fetch-depth: 0`.
