use semver::{BuildMetadata, Prerelease, Version};

use crate::classify::{self, Bump, Classified};
use crate::commit::Commit;
use crate::config::{self, Config};

/// The result of a version calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The version of the latest release tag, if there is one.
    pub previous: Option<Version>,
    pub next: Version,
    /// The bump applied to the version core (`major.minor.patch`).
    pub bump: Bump,
    /// The prerelease label from the branch rules, when one applied.
    pub prerelease_label: Option<String>,
    /// The commits that determined the bump.
    pub causes: Vec<Classified>,
}

/// Calculates the next version.
///
/// * `previous`: the latest release tag's version, or `None` without tags.
/// * `commits`: the non-merge commits since that tag (all commits if none).
/// * `branch`: the current branch, or `None` for a release build.
///
/// Without a tag the result is the configured initial version. With a tag,
/// the highest commit bump is applied (a breaking change bumps the minor
/// instead of the major below 1.0.0 when configured). If no commit matches a
/// rule, the version stays the same unless `bump_unmatched_commits` is on,
/// which bumps the patch by the number of commits.
///
/// If the branch matches a prerelease rule and there are commits since the
/// tag, `-<label>.<commit count>` is appended. A prerelease of an unchanged
/// version would sort below the existing release, so the patch is bumped
/// first in that case.
pub fn calculate(
    previous: Option<&Version>,
    commits: &[Commit],
    config: &Config,
    branch: Option<&str>,
) -> Outcome {
    let classified = classify::classify_all(commits, &config.rules);
    let highest = classified
        .iter()
        .map(|c| c.bump)
        .max()
        .unwrap_or(Bump::None);

    let (mut next, mut bump, causes) = match previous {
        None => (config.initial_version.clone(), Bump::None, Vec::new()),
        Some(prev) if highest > Bump::None => {
            let effective =
                if highest == Bump::Major && prev.major == 0 && config.breaking_bumps_minor_below_1
                {
                    Bump::Minor
                } else {
                    highest
                };
            let causes = classified
                .into_iter()
                .filter(|c| c.bump == highest)
                .collect();
            (apply(prev, effective, 1), effective, causes)
        }
        Some(prev) if config.bump_unmatched_commits && !commits.is_empty() => {
            let count = commits.len() as u64;
            (apply(prev, Bump::Patch, count), Bump::Patch, classified)
        }
        Some(prev) => (prev.clone(), Bump::None, Vec::new()),
    };

    let label = branch
        .filter(|_| !commits.is_empty())
        .and_then(|branch| config::label_for(&config.prerelease, branch));
    if let Some(label) = label {
        if let Some(prev) = previous.filter(|prev| **prev == next) {
            next = apply(prev, Bump::Patch, 1);
            bump = Bump::Patch;
        }
        next.pre = Prerelease::new(&format!("{label}.{}", commits.len()))
            .expect("labels are validated when config is loaded");
    }

    Outcome {
        previous: previous.cloned(),
        next,
        bump,
        prerelease_label: label.map(str::to_owned),
        causes,
    }
}

/// Applies `bump` to `version`, bumping the patch by `patch_count`. The
/// result is a release version: prerelease and build metadata are dropped.
fn apply(version: &Version, bump: Bump, patch_count: u64) -> Version {
    let (major, minor, patch) = (version.major, version.minor, version.patch);
    let mut next = match bump {
        Bump::Major => Version::new(major + 1, 0, 0),
        Bump::Minor => Version::new(major, minor + 1, 0),
        Bump::Patch => Version::new(major, minor, patch + patch_count),
        Bump::None => Version::new(major, minor, patch),
    };
    next.pre = Prerelease::EMPTY;
    next.build = BuildMetadata::EMPTY;
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PrereleaseRule, Source};

    fn v(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn commits(messages: &[&str]) -> Vec<Commit> {
        messages
            .iter()
            .enumerate()
            .map(|(i, message)| Commit::new(format!("hash{i}"), *message))
            .collect()
    }

    fn config(toml: &str) -> Config {
        Config::merge(&[Source::new("test", toml)]).unwrap()
    }

    fn next(
        previous: Option<&str>,
        messages: &[&str],
        config: &Config,
        branch: Option<&str>,
    ) -> String {
        calculate(previous.map(v).as_ref(), &commits(messages), config, branch)
            .next
            .to_string()
    }

    #[test]
    fn no_tags_uses_initial_version_without_bumping() {
        let defaults = Config::builtin();
        let outcome = calculate(
            None,
            &commits(&["feat: a", "fix!: b"]),
            &defaults,
            Some("main"),
        );
        assert_eq!(outcome.next, v("0.1.0"));
        assert_eq!(outcome.previous, None);
        assert_eq!(outcome.bump, Bump::None);
        assert!(outcome.causes.is_empty());
        assert_eq!(
            next(None, &[], &config("initial_version = \"1.0.0\""), None),
            "1.0.0"
        );
    }

    #[test]
    fn no_tags_on_feature_branch_is_prerelease_of_initial_version() {
        let outcome = calculate(
            None,
            &commits(&["feat: a", "docs: b"]),
            &Config::builtin(),
            Some("feature/x"),
        );
        assert_eq!(outcome.next, v("0.1.0-dev.2"));
        assert_eq!(outcome.prerelease_label.as_deref(), Some("dev"));
    }

    #[test]
    fn no_commits_since_tag_keeps_version() {
        let outcome = calculate(
            Some(&v("1.2.3")),
            &[],
            &Config::builtin(),
            Some("feature/x"),
        );
        assert_eq!(outcome.next, v("1.2.3"));
        assert_eq!(outcome.bump, Bump::None);
        assert_eq!(outcome.prerelease_label, None);
        let unmatched_on = config("bump_unmatched_commits = true");
        assert_eq!(next(Some("1.2.3"), &[], &unmatched_on, None), "1.2.3");
    }

    #[test]
    fn single_bumps() {
        let defaults = Config::builtin();
        assert_eq!(next(Some("1.2.3"), &["fix: a"], &defaults, None), "1.2.4");
        assert_eq!(next(Some("1.2.3"), &["perf: a"], &defaults, None), "1.2.4");
        assert_eq!(
            next(Some("1.2.3"), &["refactor: a"], &defaults, None),
            "1.2.4"
        );
        assert_eq!(next(Some("1.2.3"), &["feat: a"], &defaults, None), "1.3.0");
        assert_eq!(next(Some("1.2.3"), &["feat!: a"], &defaults, None), "2.0.0");
    }

    #[test]
    fn several_bumps_of_one_level_bump_once() {
        let defaults = Config::builtin();
        assert_eq!(
            next(
                Some("1.2.3"),
                &["fix: a", "fix: b", "perf: c"],
                &defaults,
                None
            ),
            "1.2.4"
        );
        assert_eq!(
            next(Some("1.2.3"), &["feat: a", "feat: b"], &defaults, None),
            "1.3.0"
        );
    }

    #[test]
    fn highest_bump_wins_and_its_commits_are_the_causes() {
        let outcome = calculate(
            Some(&v("1.2.3")),
            &commits(&["fix: a", "feat: b", "docs: c", "feat(x): d"]),
            &Config::builtin(),
            None,
        );
        assert_eq!(outcome.next, v("1.3.0"));
        assert_eq!(outcome.bump, Bump::Minor);
        let hashes: Vec<_> = outcome
            .causes
            .iter()
            .map(|c| c.commit.hash.as_str())
            .collect();
        assert_eq!(hashes, ["hash1", "hash3"]);
    }

    #[test]
    fn only_non_matching_commits_with_bump_unmatched_off() {
        let outcome = calculate(
            Some(&v("1.2.3")),
            &commits(&["docs: a", "update stuff"]),
            &Config::builtin(),
            None,
        );
        assert_eq!(outcome.next, v("1.2.3"));
        assert_eq!(outcome.bump, Bump::None);
        assert!(outcome.causes.is_empty());
    }

    #[test]
    fn only_non_matching_commits_with_bump_unmatched_on() {
        let outcome = calculate(
            Some(&v("1.2.3")),
            &commits(&["docs: a", "update stuff"]),
            &config("bump_unmatched_commits = true"),
            None,
        );
        assert_eq!(outcome.next, v("1.2.5"));
        assert_eq!(outcome.bump, Bump::Patch);
        assert_eq!(outcome.causes.len(), 2);
    }

    #[test]
    fn matching_commit_disables_bump_unmatched() {
        let on = config("bump_unmatched_commits = true");
        assert_eq!(
            next(Some("1.2.3"), &["docs: a", "fix: b", "chore: c"], &on, None),
            "1.2.4"
        );
        assert_eq!(
            next(Some("1.2.3"), &["docs: a", "feat: b"], &on, None),
            "1.3.0"
        );
    }

    #[test]
    fn mixed_commits_with_bump_unmatched_off() {
        let defaults = Config::builtin();
        assert_eq!(
            next(
                Some("1.2.3"),
                &["docs: a", "fix: b", "wip"],
                &defaults,
                None
            ),
            "1.2.4"
        );
    }

    #[test]
    fn breaking_below_1_bumps_major_by_default() {
        let defaults = Config::builtin();
        assert_eq!(next(Some("0.4.2"), &["feat!: a"], &defaults, None), "1.0.0");
        assert_eq!(
            next(
                Some("0.4.2"),
                &["fix: a\n\nBREAKING CHANGE: b"],
                &defaults,
                None
            ),
            "1.0.0"
        );
    }

    #[test]
    fn breaking_below_1_bumps_minor_when_configured() {
        let below = config("breaking_bumps_minor_below_1 = true");
        let outcome = calculate(
            Some(&v("0.4.2")),
            &commits(&["feat!: a", "fix: b"]),
            &below,
            None,
        );
        assert_eq!(outcome.next, v("0.5.0"));
        assert_eq!(outcome.bump, Bump::Minor);
        assert_eq!(outcome.causes.len(), 1);
        assert_eq!(outcome.causes[0].bump, Bump::Major);
        // Features still bump the minor below 1.0.0.
        assert_eq!(next(Some("0.4.2"), &["feat: a"], &below, None), "0.5.0");
    }

    #[test]
    fn breaking_above_1_always_bumps_major() {
        let below = config("breaking_bumps_minor_below_1 = true");
        assert_eq!(next(Some("1.2.3"), &["feat!: a"], &below, None), "2.0.0");
        assert_eq!(
            next(Some("1.2.3"), &["feat!: a"], &Config::builtin(), None),
            "2.0.0"
        );
    }

    #[test]
    fn feature_branch_gets_prerelease_with_commit_count() {
        let outcome = calculate(
            Some(&v("1.4.0")),
            &commits(&["feat: a", "docs: b", "fix: c"]),
            &Config::builtin(),
            Some("feature/login"),
        );
        assert_eq!(outcome.next, v("1.5.0-dev.3"));
        assert_eq!(outcome.bump, Bump::Minor);
        assert_eq!(outcome.prerelease_label.as_deref(), Some("dev"));
    }

    #[test]
    fn feature_branch_without_bump_bumps_patch_before_prerelease() {
        let outcome = calculate(
            Some(&v("1.4.0")),
            &commits(&["docs: a", "chore: b", "test: c"]),
            &Config::builtin(),
            Some("feature/docs"),
        );
        assert_eq!(outcome.next, v("1.4.1-dev.3"));
        assert_eq!(outcome.bump, Bump::Patch);
    }

    #[test]
    fn feature_branch_with_bump_unmatched() {
        let on = config("bump_unmatched_commits = true");
        assert_eq!(
            next(
                Some("1.4.0"),
                &["docs: a", "docs: b"],
                &on,
                Some("feature/x")
            ),
            "1.4.2-dev.2"
        );
    }

    #[test]
    fn release_branches_get_no_prerelease() {
        let defaults = Config::builtin();
        assert_eq!(
            next(Some("1.4.0"), &["feat: a"], &defaults, Some("main")),
            "1.5.0"
        );
        assert_eq!(next(Some("1.4.0"), &["feat: a"], &defaults, None), "1.5.0");
        let outcome = calculate(
            Some(&v("1.4.0")),
            &commits(&["feat: a"]),
            &defaults,
            Some("develop"),
        );
        assert_eq!(outcome.prerelease_label, None);
    }

    #[test]
    fn custom_prerelease_rules() {
        let mut custom = Config::builtin();
        custom.prerelease = vec![
            PrereleaseRule {
                branch: "release/*".into(),
                label: "rc".into(),
            },
            PrereleaseRule {
                branch: "*".into(),
                label: "snapshot".into(),
            },
        ];
        assert_eq!(
            next(Some("2.0.0"), &["fix: a"], &custom, Some("release/2.0")),
            "2.0.1-rc.1"
        );
        assert_eq!(
            next(Some("2.0.0"), &["fix: a"], &custom, Some("main")),
            "2.0.1-snapshot.1"
        );
        assert_eq!(
            next(Some("2.0.0"), &["fix: a"], &custom, Some("feature/x")),
            "2.0.1-snapshot.1"
        );
    }

    #[test]
    fn dotted_labels_are_kept() {
        let mut custom = Config::builtin();
        custom.prerelease = vec![PrereleaseRule {
            branch: "*".into(),
            label: "alpha.x".into(),
        }];
        assert_eq!(
            next(Some("1.0.0"), &["feat: a"], &custom, Some("main")),
            "1.1.0-alpha.x.1"
        );
    }

    #[test]
    fn unchanged_version_keeps_build_metadata_from_tag() {
        assert_eq!(
            next(Some("1.0.0+b5"), &["docs: a"], &Config::builtin(), None),
            "1.0.0+b5"
        );
    }

    #[test]
    fn apply_produces_release_versions() {
        let prev = v("1.2.3-rc.1+b");
        assert_eq!(apply(&prev, Bump::Major, 1), v("2.0.0"));
        assert_eq!(apply(&prev, Bump::Minor, 1), v("1.3.0"));
        assert_eq!(apply(&prev, Bump::Patch, 4), v("1.2.7"));
        assert_eq!(apply(&prev, Bump::None, 1), v("1.2.3"));
    }

    #[test]
    fn bumped_version_drops_build_metadata() {
        let defaults = Config::builtin();
        assert_eq!(
            next(Some("1.0.0+b5"), &["fix: a"], &defaults, None),
            "1.0.1"
        );
        assert_eq!(
            next(Some("1.0.0+b5"), &["docs: a"], &defaults, Some("feature/x")),
            "1.0.1-dev.1"
        );
    }
}
