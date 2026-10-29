use std::fmt;

use serde::Serialize;

use crate::commit::{self, Commit};
use crate::config::Rules;

/// How much a commit (or a set of commits) bumps the version.
/// Ordered so that the highest bump wins with `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Bump {
    None,
    Patch,
    Minor,
    Major,
}

impl fmt::Display for Bump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Bump::None => "none",
            Bump::Patch => "patch",
            Bump::Minor => "minor",
            Bump::Major => "major",
        })
    }
}

/// Classifies one commit message.
///
/// A `BREAKING CHANGE:` footer or a `!` header marker means major whatever
/// the type. Otherwise the header's type is looked up, exactly and
/// case-sensitively, in the major, minor and patch lists, in that order.
/// Messages that don't follow the header format bump nothing.
pub fn classify(message: &str, rules: &Rules) -> Bump {
    let header = commit::parse_header(message.lines().next().unwrap_or(""));
    if commit::has_breaking_footer(message) || header.as_ref().is_some_and(|h| h.breaking) {
        return Bump::Major;
    }
    let Some(header) = header else {
        return Bump::None;
    };
    let listed = |kinds: &[String]| kinds.iter().any(|kind| kind == header.kind);
    if listed(&rules.major) {
        Bump::Major
    } else if listed(&rules.minor) {
        Bump::Minor
    } else if listed(&rules.patch) {
        Bump::Patch
    } else {
        Bump::None
    }
}

/// A commit together with its classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    pub commit: Commit,
    pub bump: Bump,
}

pub fn classify_all(commits: &[Commit], rules: &Rules) -> Vec<Classified> {
    commits
        .iter()
        .map(|commit| Classified {
            bump: classify(&commit.message, rules),
            commit: commit.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn default_rules() -> Rules {
        Config::builtin().rules
    }

    #[test]
    fn bumps_are_ordered() {
        assert!(Bump::None < Bump::Patch);
        assert!(Bump::Patch < Bump::Minor);
        assert!(Bump::Minor < Bump::Major);
    }

    #[test]
    fn bump_display_and_serialize_lowercase() {
        for (bump, name) in [
            (Bump::None, "none"),
            (Bump::Patch, "patch"),
            (Bump::Minor, "minor"),
            (Bump::Major, "major"),
        ] {
            assert_eq!(bump.to_string(), name);
            assert_eq!(serde_json::to_string(&bump).unwrap(), format!("\"{name}\""));
        }
    }

    #[test]
    fn default_rule_classification() {
        let rules = default_rules();
        for (message, bump) in [
            ("feat: x", Bump::Minor),
            ("feat(api): x", Bump::Minor),
            ("fix: x", Bump::Patch),
            ("fix(parser): x", Bump::Patch),
            ("perf: x", Bump::Patch),
            ("refactor: x", Bump::Patch),
            ("docs: x", Bump::None),
            ("style: x", Bump::None),
            ("test: x", Bump::None),
            ("build: x", Bump::None),
            ("chore(release): v1.2.3", Bump::None),
            ("update readme", Bump::None),
            ("", Bump::None),
        ] {
            assert_eq!(classify(message, &rules), bump, "{message:?}");
        }
    }

    #[test]
    fn type_match_is_exact_and_case_sensitive() {
        let rules = default_rules();
        assert_eq!(classify("Feat: x", &rules), Bump::None);
        assert_eq!(classify("FIX: x", &rules), Bump::None);
        assert_eq!(classify("feature: x", &rules), Bump::None);
        assert_eq!(classify("fe: x", &rules), Bump::None);
    }

    #[test]
    fn breaking_markers_mean_major_for_any_type() {
        let rules = default_rules();
        assert_eq!(classify("feat!: x", &rules), Bump::Major);
        assert_eq!(classify("feat(api)!: x", &rules), Bump::Major);
        assert_eq!(classify("docs!: x", &rules), Bump::Major);
        assert_eq!(
            classify("fix: x\n\nBREAKING CHANGE: y", &rules),
            Bump::Major
        );
        assert_eq!(
            classify("chore: x\n\nBREAKING CHANGE: y", &rules),
            Bump::Major
        );
    }

    #[test]
    fn breaking_footer_counts_even_without_valid_header() {
        let rules = default_rules();
        assert_eq!(
            classify("rework everything\n\nBREAKING CHANGE: y", &rules),
            Bump::Major
        );
    }

    #[test]
    fn custom_rules() {
        let rules = Rules {
            major: vec!["breaking".into()],
            minor: vec!["feat".into(), "feature".into()],
            patch: vec!["fix".into()],
        };
        assert_eq!(classify("breaking: x", &rules), Bump::Major);
        assert_eq!(classify("feature: x", &rules), Bump::Minor);
        assert_eq!(classify("refactor: x", &rules), Bump::None);
        assert_eq!(classify("perf: x", &rules), Bump::None);
    }

    #[test]
    fn type_in_several_lists_takes_the_highest() {
        let rules = Rules {
            major: vec![],
            minor: vec!["feat".into()],
            patch: vec!["feat".into()],
        };
        assert_eq!(classify("feat: x", &rules), Bump::Minor);
    }

    #[test]
    fn classify_all_keeps_order() {
        let commits = vec![Commit::new("a", "fix: x"), Commit::new("b", "docs: y")];
        let classified = classify_all(&commits, &default_rules());
        assert_eq!(classified[0].commit.hash, "a");
        assert_eq!(classified[0].bump, Bump::Patch);
        assert_eq!(classified[1].commit.hash, "b");
        assert_eq!(classified[1].bump, Bump::None);
    }
}
