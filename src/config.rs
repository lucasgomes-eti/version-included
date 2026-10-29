use std::path::{Path, PathBuf};

use semver::{Prerelease, Version};
use serde::Deserialize;

use crate::branch;
use crate::env::Env;
use crate::error::Error;

/// The commented default config. It is the built-in base layer, the file
/// `versinc init` writes, and `config/default.toml` in the repository.
pub const DEFAULT_TOML: &str = include_str!("../config/default.toml");

/// Name of the project config file at the repository root.
pub const PROJECT_FILE: &str = "versinc.toml";

/// Fully resolved configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub tag_prefix: String,
    pub initial_version: Version,
    pub bump_unmatched_commits: bool,
    pub breaking_bumps_minor_below_1: bool,
    pub rules: Rules,
    pub prerelease: Vec<PrereleaseRule>,
}

/// Commit types that trigger each bump level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    pub major: Vec<String>,
    pub minor: Vec<String>,
    pub patch: Vec<String>,
}

/// Maps a branch glob pattern to a prerelease label.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrereleaseRule {
    pub branch: String,
    pub label: String,
}

/// One config file as written: every key optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Layer {
    tag_prefix: Option<String>,
    initial_version: Option<String>,
    bump_unmatched_commits: Option<bool>,
    breaking_bumps_minor_below_1: Option<bool>,
    rules: Option<RulesLayer>,
    prerelease: Option<Vec<PrereleaseRule>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesLayer {
    major: Option<Vec<String>>,
    minor: Option<Vec<String>>,
    patch: Option<Vec<String>>,
}

/// A config file's display name and contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub name: String,
    pub text: String,
}

impl Source {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        Source {
            name: name.into(),
            text: text.into(),
        }
    }
}

impl Config {
    /// The built-in defaults, parsed from [`DEFAULT_TOML`].
    pub fn builtin() -> Config {
        Config::merge(&[]).expect("built-in default config is valid")
    }

    /// Parses and validates each source, then merges them over the built-in
    /// defaults key by key. Later sources win; lists replace earlier lists.
    pub fn merge(sources: &[Source]) -> Result<Config, Error> {
        let mut merged = parse_layer(&Source::new("<built-in defaults>", DEFAULT_TOML))?;
        for source in sources {
            merged.overlay(parse_layer(source)?);
        }
        Ok(merged.resolve())
    }
}

impl Layer {
    fn overlay(&mut self, other: Layer) {
        fn set<T>(slot: &mut Option<T>, value: Option<T>) {
            if value.is_some() {
                *slot = value;
            }
        }
        set(&mut self.tag_prefix, other.tag_prefix);
        set(&mut self.initial_version, other.initial_version);
        set(
            &mut self.bump_unmatched_commits,
            other.bump_unmatched_commits,
        );
        set(
            &mut self.breaking_bumps_minor_below_1,
            other.breaking_bumps_minor_below_1,
        );
        set(&mut self.prerelease, other.prerelease);
        if let Some(other_rules) = other.rules {
            let rules = self.rules.get_or_insert_with(RulesLayer::default);
            set(&mut rules.major, other_rules.major);
            set(&mut rules.minor, other_rules.minor);
            set(&mut rules.patch, other_rules.patch);
        }
    }

    /// Converts the merged layers into a `Config`. Every key is present
    /// because the built-in defaults define all of them, and every value was
    /// validated when its layer was parsed.
    fn resolve(self) -> Config {
        const MISSING: &str = "built-in defaults define every key";
        let rules = self.rules.expect(MISSING);
        Config {
            tag_prefix: self.tag_prefix.expect(MISSING),
            initial_version: Version::parse(&self.initial_version.expect(MISSING))
                .expect("validated when parsed"),
            bump_unmatched_commits: self.bump_unmatched_commits.expect(MISSING),
            breaking_bumps_minor_below_1: self.breaking_bumps_minor_below_1.expect(MISSING),
            rules: Rules {
                major: rules.major.expect(MISSING),
                minor: rules.minor.expect(MISSING),
                patch: rules.patch.expect(MISSING),
            },
            prerelease: self.prerelease.expect(MISSING),
        }
    }
}

fn parse_layer(source: &Source) -> Result<Layer, Error> {
    let layer: Layer = toml::from_str(&source.text)
        .map_err(|e| Error::config(&source.name, e.to_string().trim_end()))?;
    validate(&layer).map_err(|message| Error::config(&source.name, message))?;
    Ok(layer)
}

fn validate(layer: &Layer) -> Result<(), String> {
    if let Some(version) = &layer.initial_version {
        let parsed = Version::parse(version).map_err(|e| {
            format!("key `initial_version`: {version:?} is not a valid SemVer version ({e})")
        })?;
        if !parsed.pre.is_empty() || !parsed.build.is_empty() {
            return Err(format!(
                "key `initial_version`: {version:?} must not have a prerelease or build suffix"
            ));
        }
    }
    if let Some(rules) = &layer.rules {
        for (key, kinds) in [
            ("major", &rules.major),
            ("minor", &rules.minor),
            ("patch", &rules.patch),
        ] {
            for (i, kind) in kinds.iter().flatten().enumerate() {
                if kind.is_empty() || kind.chars().any(char::is_whitespace) {
                    return Err(format!(
                        "key `rules.{key}[{i}]`: {kind:?} is not a valid commit type"
                    ));
                }
            }
        }
    }
    for (i, rule) in layer.prerelease.iter().flatten().enumerate() {
        if rule.branch.is_empty() {
            return Err(format!(
                "key `prerelease[{i}].branch`: pattern must not be empty"
            ));
        }
        if rule.label.is_empty() {
            return Err(format!(
                "key `prerelease[{i}].label`: label must not be empty"
            ));
        }
        if let Err(e) = Prerelease::new(&rule.label) {
            return Err(format!(
                "key `prerelease[{i}].label`: {:?} is not a valid SemVer prerelease identifier ({e})",
                rule.label
            ));
        }
    }
    Ok(())
}

/// Path of the global config file: `$XDG_CONFIG_HOME/version-included/config.toml`,
/// falling back to `$HOME/.config/version-included/config.toml`.
pub fn global_path(env: &dyn Env) -> Option<PathBuf> {
    let base = env
        .var("XDG_CONFIG_HOME")
        .filter(|dir| Path::new(dir).is_absolute())
        .map(PathBuf::from)
        .or_else(|| {
            env.var("HOME")
                .filter(|home| !home.is_empty())
                .map(|home| Path::new(&home).join(".config"))
        })?;
    Some(base.join("version-included").join("config.toml"))
}

/// Path of the project config file for a repository root.
pub fn project_path(repo_root: &Path) -> PathBuf {
    repo_root.join(PROJECT_FILE)
}

/// The prerelease label for `branch`: the label of the first rule whose
/// pattern matches.
pub fn label_for<'a>(rules: &'a [PrereleaseRule], branch: &str) -> Option<&'a str> {
    rules
        .iter()
        .find(|rule| branch::glob_match(&rule.branch, branch))
        .map(|rule| rule.label.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::tests::FakeEnv;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn err(text: &str) -> String {
        Config::merge(&[Source::new("test.toml", text)])
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn embedded_default_matches_repo_file() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/config/default.toml");
        let on_disk = std::fs::read_to_string(path).unwrap();
        assert_eq!(DEFAULT_TOML, on_disk);
    }

    #[test]
    fn builtin_defaults() {
        assert_eq!(
            Config::builtin(),
            Config {
                tag_prefix: "v".into(),
                initial_version: Version::new(0, 1, 0),
                bump_unmatched_commits: false,
                breaking_bumps_minor_below_1: false,
                rules: Rules {
                    major: vec![],
                    minor: strings(&["feat"]),
                    patch: strings(&["fix", "perf", "refactor"]),
                },
                prerelease: vec![PrereleaseRule {
                    branch: "feature/*".into(),
                    label: "dev".into(),
                }],
            }
        );
    }

    #[test]
    fn empty_file_keeps_defaults() {
        assert_eq!(
            Config::merge(&[Source::new("a", "")]).unwrap(),
            Config::builtin()
        );
    }

    #[test]
    fn later_layers_win_key_by_key() {
        let config = Config::merge(&[
            Source::new(
                "global",
                "tag_prefix = \"release-\"\n[rules]\npatch = [\"fix\"]",
            ),
            Source::new(
                "project",
                "bump_unmatched_commits = true\n[rules]\nminor = [\"feat\", \"refactor\"]",
            ),
            Source::new("cli", "tag_prefix = \"\""),
        ])
        .unwrap();
        assert_eq!(config.tag_prefix, "");
        assert!(config.bump_unmatched_commits);
        assert!(!config.breaking_bumps_minor_below_1);
        assert_eq!(config.rules.patch, strings(&["fix"]));
        assert_eq!(config.rules.minor, strings(&["feat", "refactor"]));
        assert_eq!(config.rules.major, Vec::<String>::new());
    }

    #[test]
    fn lists_replace_instead_of_appending() {
        let config = Config::merge(&[
            Source::new("a", "[rules]\npatch = [\"fix\"]"),
            Source::new(
                "b",
                "[[prerelease]]\nbranch = \"release/*\"\nlabel = \"rc\"",
            ),
        ])
        .unwrap();
        assert_eq!(config.rules.patch, strings(&["fix"]));
        assert_eq!(
            config.prerelease,
            vec![PrereleaseRule {
                branch: "release/*".into(),
                label: "rc".into(),
            }]
        );
    }

    #[test]
    fn empty_prerelease_list_disables_prereleases() {
        let config = Config::merge(&[Source::new("a", "prerelease = []")]).unwrap();
        assert!(config.prerelease.is_empty());
    }

    #[test]
    fn all_keys_can_be_set() {
        let config = Config::merge(&[Source::new(
            "a",
            r#"
            tag_prefix = "ver"
            initial_version = "1.0.0"
            bump_unmatched_commits = true
            breaking_bumps_minor_below_1 = true
            [rules]
            major = ["breaking"]
            "#,
        )])
        .unwrap();
        assert_eq!(config.tag_prefix, "ver");
        assert_eq!(config.initial_version, Version::new(1, 0, 0));
        assert!(config.bump_unmatched_commits);
        assert!(config.breaking_bumps_minor_below_1);
        assert_eq!(config.rules.major, strings(&["breaking"]));
    }

    #[test]
    fn unknown_key_names_file_and_key() {
        let message = err("tag_prefx = \"v\"");
        assert!(
            message.starts_with("invalid config file test.toml:"),
            "{message}"
        );
        assert!(message.contains("tag_prefx"), "{message}");
    }

    #[test]
    fn unknown_nested_key_is_rejected() {
        assert!(err("[rules]\nminro = []").contains("minro"));
        assert!(err("[[prerelease]]\nbranch = \"x\"\nlabel = \"a\"\ncolor = 1").contains("color"));
    }

    #[test]
    fn wrong_type_names_key() {
        let message = err("bump_unmatched_commits = \"yes\"");
        assert!(message.contains("bump_unmatched_commits"), "{message}");
    }

    #[test]
    fn syntax_error_names_file() {
        assert!(err("tag_prefix = ").starts_with("invalid config file test.toml:"));
    }

    #[test]
    fn invalid_initial_version() {
        let message = err("initial_version = \"1.2\"");
        assert!(message.contains("key `initial_version`"), "{message}");
        assert!(err("initial_version = \"1.0.0-rc.1\"").contains("prerelease or build"));
        assert!(err("initial_version = \"1.0.0+b\"").contains("prerelease or build"));
    }

    #[test]
    fn invalid_rule_types() {
        assert!(err("[rules]\nminor = [\"feat\", \"\"]").contains("key `rules.minor[1]`"));
        assert!(err("[rules]\npatch = [\"a b\"]").contains("key `rules.patch[0]`"));
        assert!(err("[rules]\nmajor = [\" \"]").contains("key `rules.major[0]`"));
    }

    #[test]
    fn invalid_prerelease_rules() {
        assert!(
            err("[[prerelease]]\nbranch = \"\"\nlabel = \"dev\"")
                .contains("key `prerelease[0].branch`")
        );
        let message = err(
            "[[prerelease]]\nbranch = \"x\"\nlabel = \"dev\"\n[[prerelease]]\nbranch = \"y\"\nlabel = \"a_b\"",
        );
        assert!(message.contains("key `prerelease[1].label`"), "{message}");
        assert!(
            err("[[prerelease]]\nbranch = \"x\"\nlabel = \"\"")
                .contains("key `prerelease[0].label`")
        );
        assert!(
            err("[[prerelease]]\nbranch = \"x\"\nlabel = \"01\"")
                .contains("key `prerelease[0].label`")
        );
        assert!(err("[[prerelease]]\nbranch = \"x\"").contains("label"));
    }

    #[test]
    fn first_error_stops_merging() {
        let message = Config::merge(&[
            Source::new("good.toml", ""),
            Source::new("bad.toml", "x = 1"),
        ])
        .unwrap_err()
        .to_string();
        assert!(message.contains("bad.toml"), "{message}");
    }

    #[test]
    fn global_path_prefers_absolute_xdg_config_home() {
        let env = FakeEnv::new(&[("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/u")]);
        assert_eq!(
            global_path(&env),
            Some(PathBuf::from("/xdg/version-included/config.toml"))
        );
    }

    #[test]
    fn global_path_falls_back_to_home() {
        for xdg in [None, Some(""), Some("relative/dir")] {
            let mut vars = vec![("HOME", "/home/u")];
            if let Some(xdg) = xdg {
                vars.push(("XDG_CONFIG_HOME", xdg));
            }
            assert_eq!(
                global_path(&FakeEnv::new(&vars)),
                Some(PathBuf::from(
                    "/home/u/.config/version-included/config.toml"
                )),
                "{xdg:?}"
            );
        }
    }

    #[test]
    fn global_path_needs_home_or_xdg() {
        assert_eq!(global_path(&FakeEnv::new(&[])), None);
        assert_eq!(global_path(&FakeEnv::new(&[("HOME", "")])), None);
    }

    #[test]
    fn project_path_is_at_repo_root() {
        assert_eq!(
            project_path(Path::new("/repo")),
            PathBuf::from("/repo/versinc.toml")
        );
    }

    #[test]
    fn label_for_uses_first_matching_rule() {
        let rules = [
            PrereleaseRule {
                branch: "feature/special".into(),
                label: "special".into(),
            },
            PrereleaseRule {
                branch: "feature/*".into(),
                label: "dev".into(),
            },
            PrereleaseRule {
                branch: "release/*".into(),
                label: "rc".into(),
            },
        ];
        assert_eq!(label_for(&rules, "feature/special"), Some("special"));
        assert_eq!(label_for(&rules, "feature/login"), Some("dev"));
        assert_eq!(label_for(&rules, "release/2.0"), Some("rc"));
        assert_eq!(label_for(&rules, "main"), None);
        assert_eq!(label_for(&[], "feature/x"), None);
    }
}
