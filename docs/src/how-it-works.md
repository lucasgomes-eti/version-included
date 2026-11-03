# How the version is calculated

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

## Commit rules

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

The types for each bump level can be changed in the [configuration](configuration.md#reference).

Examples, starting from tag `v1.2.3`:

| Commits since the tag          | Next version                                              |
| ------------------------------ | --------------------------------------------------------- |
| none                           | `1.2.3`                                                   |
| `fix: …`                       | `1.2.4`                                                   |
| `fix: …`, `feat: …`, `docs: …` | `1.3.0`                                                   |
| `feat(api)!: …`                | `2.0.0`                                                   |
| `docs: …`, `chore: …`          | `1.2.3` (or `1.2.5` with `bump_unmatched_commits = true`) |

## Edge cases

- **Only non-matching commits.** By default the version stays the same. With
  `bump_unmatched_commits = true`, the patch is bumped by the number of commits since the tag
  (`1.2.3` + 2 commits → `1.2.5`). As soon as one commit matches a rule, the normal rules apply
  and this option is ignored.
- **Breaking changes below 1.0.0.** By default they bump the major (`0.4.2` → `1.0.0`). With
  `breaking_bumps_minor_below_1 = true` they bump the minor instead while the major is 0
  (`0.4.2` → `0.5.0`).

## Branches and prereleases

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
