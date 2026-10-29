use semver::Version;

/// A git tag that parses as a release version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionTag {
    pub name: String,
    pub version: Version,
}

/// Parses a tag name as a release version after stripping `prefix`.
///
/// Returns `None` for tags without the prefix, tags that aren't valid
/// SemVer 2.0.0, and prerelease tags (`v1.2.0-rc.1`), which are never used
/// as a base version.
pub fn parse(name: &str, prefix: &str) -> Option<Version> {
    let version = Version::parse(name.strip_prefix(prefix)?).ok()?;
    version.pre.is_empty().then_some(version)
}

/// The tag with the highest SemVer precedence among `names`.
///
/// When tags differ only in build metadata (`v1.0.0+a`, `v1.0.0+b`), the
/// lexicographically last one wins so the choice is deterministic.
pub fn latest<S: AsRef<str>>(names: &[S], prefix: &str) -> Option<VersionTag> {
    names
        .iter()
        .filter_map(|name| {
            let name = name.as_ref();
            parse(name, prefix).map(|version| VersionTag {
                name: name.to_owned(),
                version,
            })
        })
        .max_by(|a, b| a.version.cmp(&b.version).then_with(|| a.name.cmp(&b.name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prefixed_release_tags() {
        assert_eq!(parse("v1.2.3", "v"), Some(Version::new(1, 2, 3)));
        assert_eq!(parse("1.2.3", ""), Some(Version::new(1, 2, 3)));
        assert_eq!(
            parse("release-2.0.0", "release-"),
            Some(Version::new(2, 0, 0))
        );
        assert_eq!(
            parse("v1.2.3+build.7", "v").map(|v| v.build.to_string()),
            Some("build.7".into())
        );
    }

    #[test]
    fn ignores_tags_that_are_not_release_versions() {
        for name in [
            "1.2.3",
            "v1.2",
            "v1",
            "vfoo",
            "v01.2.3",
            "v1.2.3.4",
            "V1.2.3",
            "release-5",
            "v1.2.0-rc.1",
            "",
        ] {
            assert_eq!(parse(name, "v"), None, "{name:?}");
        }
        assert_eq!(parse("v1.2.3", ""), None);
    }

    #[test]
    fn latest_picks_highest_precedence_not_list_order() {
        let tags = ["v1.10.0", "v1.9.0", "v2.0.0-rc.1", "junk", "v1.2.3"];
        assert_eq!(
            latest(&tags, "v"),
            Some(VersionTag {
                name: "v1.10.0".into(),
                version: Version::new(1, 10, 0),
            })
        );
    }

    #[test]
    fn latest_with_build_metadata_is_deterministic() {
        let forward = latest(&["v1.0.0+a", "v1.0.0+b"], "v").unwrap();
        let backward = latest(&["v1.0.0+b", "v1.0.0+a"], "v").unwrap();
        assert_eq!(forward, backward);
    }

    #[test]
    fn latest_without_release_tags() {
        assert_eq!(latest::<&str>(&[], "v"), None);
        assert_eq!(latest(&["nightly", "v3.0.0-beta"], "v"), None);
    }
}
