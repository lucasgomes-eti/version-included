use semver::Version;
use serde::Serialize;

use crate::classify::Bump;
use crate::version::Outcome;

/// Formats a version, optionally with the tag prefix.
pub fn version_string(version: &Version, prefix: Option<&str>) -> String {
    format!("{}{version}", prefix.unwrap_or(""))
}

#[derive(Serialize)]
struct Report<'a> {
    previous_version: Option<String>,
    next_version: String,
    bump: Bump,
    branch: Option<&'a str>,
    prerelease_label: Option<&'a str>,
    config_files: &'a [String],
    commits: Vec<ReportCommit<'a>>,
}

#[derive(Serialize)]
struct ReportCommit<'a> {
    hash: &'a str,
    subject: &'a str,
    bump: Bump,
}

/// Renders the calculation as pretty-printed JSON. `prefix`, when given,
/// is added to both versions.
pub fn json(
    outcome: &Outcome,
    branch: Option<&str>,
    config_files: &[String],
    prefix: Option<&str>,
) -> String {
    let report = Report {
        previous_version: outcome
            .previous
            .as_ref()
            .map(|version| version_string(version, prefix)),
        next_version: version_string(&outcome.next, prefix),
        bump: outcome.bump,
        branch,
        prerelease_label: outcome.prerelease_label.as_deref(),
        config_files,
        commits: outcome
            .causes
            .iter()
            .map(|c| ReportCommit {
                hash: &c.commit.hash,
                subject: c.commit.subject(),
                bump: c.bump,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&report).expect("report serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classify::Classified;
    use crate::commit::Commit;
    use serde_json::json;

    fn outcome() -> Outcome {
        Outcome {
            previous: Some(Version::new(1, 4, 0)),
            next: Version::parse("1.5.0-dev.2").unwrap(),
            bump: Bump::Minor,
            prerelease_label: Some("dev".into()),
            causes: vec![Classified {
                commit: Commit::new("abc", "feat: \"quoted\"\n\nbody"),
                bump: Bump::Minor,
            }],
        }
    }

    #[test]
    fn version_string_with_and_without_prefix() {
        let version = Version::new(1, 2, 3);
        assert_eq!(version_string(&version, None), "1.2.3");
        assert_eq!(version_string(&version, Some("v")), "v1.2.3");
    }

    #[test]
    fn json_report() {
        let text = json(
            &outcome(),
            Some("feature/x"),
            &["/repo/versinc.toml".into()],
            None,
        );
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value,
            json!({
                "previous_version": "1.4.0",
                "next_version": "1.5.0-dev.2",
                "bump": "minor",
                "branch": "feature/x",
                "prerelease_label": "dev",
                "config_files": ["/repo/versinc.toml"],
                "commits": [
                    {"hash": "abc", "subject": "feat: \"quoted\"", "bump": "minor"}
                ]
            })
        );
    }

    #[test]
    fn json_report_with_prefix_and_nulls() {
        let mut outcome = outcome();
        outcome.previous = None;
        outcome.prerelease_label = None;
        outcome.causes.clear();
        let value: serde_json::Value =
            serde_json::from_str(&json(&outcome, None, &[], Some("v"))).unwrap();
        assert_eq!(value["previous_version"], serde_json::Value::Null);
        assert_eq!(value["next_version"], "v1.5.0-dev.2");
        assert_eq!(value["branch"], serde_json::Value::Null);
        assert_eq!(value["prerelease_label"], serde_json::Value::Null);
        assert_eq!(value["config_files"], json!([]));
        assert_eq!(value["commits"], json!([]));
    }
}
