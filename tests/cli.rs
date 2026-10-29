mod support;

use std::fs;

use predicates::prelude::*;
use predicates::str::contains;
use support::Sandbox;

const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

fn tagged(version: &str) -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.commit("feat: initial").tag(version);
    sandbox
}

// Tags

#[test]
fn no_tags_prints_initial_version() {
    let sandbox = Sandbox::new();
    sandbox.commit("feat: a").commit("fix: b");
    assert_eq!(sandbox.version(), "0.1.0\n");
}

#[test]
fn no_tags_uses_configured_initial_version() {
    let sandbox = Sandbox::new();
    sandbox.commit("feat: a");
    sandbox.write("repo/versinc.toml", "initial_version = \"1.0.0\"");
    assert_eq!(sandbox.version(), "1.0.0\n");
}

#[test]
fn repository_without_commits_prints_initial_version() {
    let sandbox = Sandbox::new();
    sandbox
        .versinc()
        .assert()
        .success()
        .stdout("0.1.0\n")
        .stderr("");
}

#[test]
fn no_commits_since_last_tag() {
    let sandbox = tagged("v1.2.3");
    assert_eq!(sandbox.version(), "1.2.3\n");
}

#[test]
fn multiple_tags_on_one_commit_use_the_highest() {
    let sandbox = Sandbox::new();
    sandbox.commit("feat: a").tag("v1.1.0").tag("v1.0.0");
    sandbox.commit("fix: b");
    assert_eq!(sandbox.version(), "1.1.1\n");
}

#[test]
fn non_semver_and_prerelease_tags_are_ignored() {
    let sandbox = tagged("v1.0.0");
    sandbox.commit("fix: a");
    for tag in ["release-5", "v2", "vfoo", "9.0.0", "v9.0.0-rc.1", "v01.0.0"] {
        sandbox.tag(tag);
    }
    assert_eq!(sandbox.version(), "1.0.1\n");
}

#[test]
fn tags_not_reachable_from_head_are_ignored() {
    let sandbox = tagged("v1.0.0");
    sandbox.git(&["checkout", "--quiet", "-b", "other"]);
    sandbox.commit("feat: elsewhere").tag("v5.0.0");
    sandbox.git(&["checkout", "--quiet", "main"]);
    sandbox.commit("fix: here");
    assert_eq!(sandbox.version(), "1.0.1\n");
}

#[test]
fn custom_tag_prefix() {
    let sandbox = tagged("release-2.0.0");
    sandbox.tag("v9.0.0").commit("feat: a");
    sandbox.write("repo/versinc.toml", "tag_prefix = \"release-\"");
    assert_eq!(sandbox.version(), "2.1.0\n");
    sandbox
        .versinc()
        .arg("--with-prefix")
        .assert()
        .success()
        .stdout("release-2.1.0\n");
}

// Commit classification

#[test]
fn bump_levels() {
    for (message, expected) in [
        ("fix: a", "1.2.4"),
        ("perf(db): a", "1.2.4"),
        ("refactor: a", "1.2.4"),
        ("feat(api): a", "1.3.0"),
        ("feat!: a", "2.0.0"),
        ("feat(api)!: a", "2.0.0"),
        ("docs: a\n\nBREAKING CHANGE: removed the old flag", "2.0.0"),
    ] {
        let sandbox = tagged("v1.2.3");
        sandbox.commit(message);
        assert_eq!(sandbox.version(), format!("{expected}\n"), "{message:?}");
    }
}

#[test]
fn only_non_matching_commits_with_bump_unmatched_off() {
    let sandbox = tagged("v1.2.3");
    sandbox.commit("docs: a").commit("update things");
    assert_eq!(sandbox.version(), "1.2.3\n");
}

#[test]
fn only_non_matching_commits_with_bump_unmatched_on() {
    let sandbox = tagged("v1.2.3");
    sandbox.commit("docs: a").commit("update things");
    sandbox.write("repo/versinc.toml", "bump_unmatched_commits = true");
    assert_eq!(sandbox.version(), "1.2.5\n");
}

#[test]
fn mixed_matching_and_non_matching_commits() {
    let sandbox = tagged("v1.2.3");
    sandbox
        .commit("docs: a")
        .commit("fix: b")
        .commit("wip")
        .commit("feat: c")
        .commit("chore: d");
    assert_eq!(sandbox.version(), "1.3.0\n");
    // A matching commit means bump_unmatched_commits doesn't apply.
    sandbox.write("repo/versinc.toml", "bump_unmatched_commits = true");
    assert_eq!(sandbox.version(), "1.3.0\n");
}

#[test]
fn breaking_change_below_1_bumps_major_by_default() {
    let sandbox = tagged("v0.4.2");
    sandbox.commit("feat!: new api");
    assert_eq!(sandbox.version(), "1.0.0\n");
}

#[test]
fn breaking_change_below_1_bumps_minor_when_configured() {
    let sandbox = tagged("v0.4.2");
    sandbox.commit("fix: a\n\nBREAKING CHANGE: b");
    sandbox.write("repo/versinc.toml", "breaking_bumps_minor_below_1 = true");
    assert_eq!(sandbox.version(), "0.5.0\n");
}

#[test]
fn breaking_change_above_1_bumps_major() {
    let sandbox = tagged("v1.4.2");
    sandbox.commit("feat(api)!: drop v1");
    sandbox.write("repo/versinc.toml", "breaking_bumps_minor_below_1 = true");
    assert_eq!(sandbox.version(), "2.0.0\n");
}

#[test]
fn merge_commits_are_skipped() {
    let sandbox = tagged("v1.0.0");
    sandbox.git(&["checkout", "--quiet", "-b", "topic"]);
    sandbox.commit("fix: on topic");
    sandbox.git(&["checkout", "--quiet", "main"]);
    sandbox.commit("docs: on main");
    sandbox.git(&[
        "merge",
        "--quiet",
        "--no-ff",
        "topic",
        "-m",
        "feat!: merge topic",
    ]);
    assert_eq!(sandbox.version(), "1.0.1\n");
}

// Branches

#[test]
fn feature_branch_gets_prerelease() {
    let sandbox = tagged("v1.4.0");
    sandbox.git(&["checkout", "--quiet", "-b", "feature/login"]);
    sandbox.commit("feat: a").commit("docs: b");
    assert_eq!(sandbox.version(), "1.5.0-dev.2\n");
}

#[test]
fn feature_branch_without_bump_bumps_patch() {
    let sandbox = tagged("v1.4.0");
    sandbox.git(&["checkout", "--quiet", "-b", "feature/docs"]);
    sandbox.commit("docs: a");
    assert_eq!(sandbox.version(), "1.4.1-dev.1\n");
}

#[test]
fn feature_branch_at_tag_is_the_release() {
    let sandbox = tagged("v1.4.0");
    sandbox.git(&["checkout", "--quiet", "-b", "feature/new"]);
    assert_eq!(sandbox.version(), "1.4.0\n");
}

#[test]
fn custom_branch_patterns_and_labels() {
    let sandbox = tagged("v2.0.0");
    sandbox.write(
        "repo/versinc.toml",
        "[[prerelease]]\nbranch = \"release/*\"\nlabel = \"rc\"\n\n\
         [[prerelease]]\nbranch = \"develop\"\nlabel = \"beta\"\n",
    );
    sandbox.git(&["checkout", "--quiet", "-b", "release/2.1"]);
    sandbox.commit("feat: a");
    assert_eq!(sandbox.version(), "2.1.0-rc.1\n");
    sandbox.git(&["checkout", "--quiet", "-b", "develop"]);
    sandbox.commit("fix: b");
    assert_eq!(sandbox.version(), "2.1.0-beta.2\n");
    // feature/* was replaced by the project's list.
    sandbox.git(&["checkout", "--quiet", "-b", "feature/x"]);
    assert_eq!(sandbox.version(), "2.1.0\n");
}

#[test]
fn detached_head_without_github_head_ref_warns() {
    let sandbox = tagged("v1.0.0");
    sandbox.git(&["checkout", "--quiet", "-b", "feature/x"]);
    sandbox.commit("feat: a");
    sandbox.git(&["checkout", "--quiet", "--detach"]);
    sandbox
        .versinc()
        .assert()
        .success()
        .stdout("1.1.0\n")
        .stderr(contains("warning: cannot determine the current branch"));
}

#[test]
fn detached_head_with_github_head_ref() {
    let sandbox = tagged("v1.0.0");
    sandbox.commit("feat: a");
    sandbox.git(&["checkout", "--quiet", "--detach"]);
    sandbox
        .versinc()
        .env("GITHUB_HEAD_REF", "feature/pr-branch")
        .assert()
        .success()
        .stdout("1.1.0-dev.1\n")
        .stderr("");
}

#[test]
fn detached_head_with_empty_github_head_ref_warns() {
    let sandbox = tagged("v1.0.0");
    sandbox.commit("feat: a");
    sandbox.git(&["checkout", "--quiet", "--detach"]);
    sandbox
        .versinc()
        .env("GITHUB_HEAD_REF", "")
        .assert()
        .success()
        .stdout("1.1.0\n")
        .stderr(contains("warning:"));
}

// Configuration

#[test]
fn config_layers_merge_key_by_key() {
    let sandbox = Sandbox::new();
    sandbox
        .commit("feat: a")
        .tag("v1.0.0")
        .tag("release-2.0.0")
        .commit("refactor: b");
    let subdir = sandbox.repo.join("src/nested");
    fs::create_dir_all(&subdir).unwrap();
    let run = |extra: &[&str]| {
        let output = sandbox.versinc_in(&subdir).args(extra).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    assert_eq!(run(&[]), "1.0.1\n");

    // Global: refactor no longer bumps the patch.
    fs::create_dir_all(sandbox.global_config().parent().unwrap()).unwrap();
    fs::write(sandbox.global_config(), "[rules]\npatch = [\"fix\"]\n").unwrap();
    assert_eq!(run(&[]), "1.0.0\n");

    // Project (found at the repo root from a subdirectory): refactor is a
    // minor bump; the global patch list still applies.
    sandbox.write(
        "repo/versinc.toml",
        "[rules]\nminor = [\"feat\", \"refactor\"]\n",
    );
    assert_eq!(run(&[]), "1.1.0\n");

    // --config wins over both.
    let cli = sandbox.write("ci/versinc.toml", "tag_prefix = \"release-\"\n");
    let cli_arg = cli.to_str().unwrap();
    assert_eq!(run(&["--config", cli_arg]), "2.1.0\n");

    let json: serde_json::Value =
        serde_json::from_str(&run(&["--config", cli_arg, "--format", "json"])).unwrap();
    assert_eq!(
        json["config_files"],
        serde_json::json!([
            sandbox.global_config().to_str().unwrap(),
            sandbox.repo.join("versinc.toml").to_str().unwrap(),
            cli_arg,
        ])
    );
}

#[test]
fn xdg_config_home_overrides_home() {
    let sandbox = tagged("v1.0.0");
    sandbox.commit("refactor: a");
    sandbox.write("xdg/version-included/config.toml", "[rules]\npatch = []\n");
    sandbox
        .versinc()
        .env("XDG_CONFIG_HOME", sandbox.root.join("xdg"))
        .assert()
        .success()
        .stdout("1.0.0\n");
}

#[test]
fn missing_explicit_config_file() {
    let sandbox = tagged("v1.0.0");
    sandbox
        .versinc()
        .args(["--config", "missing.toml"])
        .assert()
        .code(1)
        .stdout("")
        .stderr("error: config file missing.toml does not exist\n");
}

#[test]
fn invalid_config_names_file_and_key() {
    let sandbox = tagged("v1.0.0");
    let path = sandbox.write("repo/versinc.toml", "[rules]\nminro = [\"feat\"]\n");
    sandbox
        .versinc()
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains(format!(
            "error: invalid config file {}",
            path.display()
        )))
        .stderr(contains("minro"));

    sandbox.write("repo/versinc.toml", "initial_version = \"one\"\n");
    sandbox
        .versinc()
        .assert()
        .code(1)
        .stderr(contains("key `initial_version`"));

    sandbox.write(
        "repo/versinc.toml",
        "[[prerelease]]\nbranch = \"x\"\nlabel = \"bad label\"\n",
    );
    sandbox
        .versinc()
        .assert()
        .code(1)
        .stderr(contains("key `prerelease[0].label`"));
}

#[test]
fn invalid_global_config_names_global_file() {
    let sandbox = tagged("v1.0.0");
    fs::create_dir_all(sandbox.global_config().parent().unwrap()).unwrap();
    fs::write(sandbox.global_config(), "tag_prefix = 1\n").unwrap();
    sandbox
        .versinc()
        .assert()
        .code(1)
        .stderr(contains(sandbox.global_config().to_str().unwrap()))
        .stderr(contains("tag_prefix"));
}

// init

#[test]
fn init_writes_default_config_at_repo_root() {
    let sandbox = Sandbox::new();
    let subdir = sandbox.repo.join("sub");
    fs::create_dir_all(&subdir).unwrap();
    let target = sandbox.repo.join("versinc.toml");
    sandbox
        .versinc_in(&subdir)
        .arg("init")
        .assert()
        .success()
        .stdout("")
        .stderr(format!("wrote {}\n", target.display()));
    assert_eq!(fs::read_to_string(&target).unwrap(), DEFAULT_CONFIG);
    // The written file is a valid config that reproduces the defaults.
    sandbox
        .commit("feat: a")
        .tag("v1.0.0")
        .commit("refactor: b");
    assert_eq!(sandbox.version(), "1.0.1\n");
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let sandbox = Sandbox::new();
    let target = sandbox.write("repo/versinc.toml", "tag_prefix = \"\"\n");
    sandbox
        .versinc()
        .arg("init")
        .assert()
        .code(1)
        .stderr(contains("already exists; pass --force to overwrite it"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "tag_prefix = \"\"\n");

    sandbox
        .versinc()
        .args(["init", "--force"])
        .assert()
        .success();
    assert_eq!(fs::read_to_string(&target).unwrap(), DEFAULT_CONFIG);
}

#[test]
fn init_global_creates_directories() {
    let sandbox = Sandbox::new();
    let outside = sandbox.root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    sandbox
        .versinc_in(&outside)
        .args(["init", "--global"])
        .assert()
        .success()
        .stderr(format!("wrote {}\n", sandbox.global_config().display()));
    assert_eq!(
        fs::read_to_string(sandbox.global_config()).unwrap(),
        DEFAULT_CONFIG
    );

    sandbox
        .versinc_in(&outside)
        .args(["init", "--global"])
        .assert()
        .code(1)
        .stderr(contains("already exists"));
    sandbox
        .versinc_in(&outside)
        .args(["init", "--global", "--force"])
        .assert()
        .success();
}

#[test]
fn init_outside_repo_fails() {
    let sandbox = Sandbox::new();
    let outside = sandbox.root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    sandbox
        .versinc_in(&outside)
        .arg("init")
        .assert()
        .code(1)
        .stderr("error: not inside a git repository\n");
}

// Output

#[test]
fn with_prefix_flag() {
    let sandbox = tagged("v1.0.0");
    sandbox.commit("feat: a");
    sandbox
        .versinc()
        .arg("--with-prefix")
        .assert()
        .success()
        .stdout("v1.1.0\n");
}

#[test]
fn output_file() {
    let sandbox = tagged("v1.0.0");
    sandbox.commit("fix: a");
    let file = sandbox.root.join("version.txt");
    sandbox
        .versinc()
        .args(["--with-prefix", "--output"])
        .arg(&file)
        .assert()
        .success()
        .stdout("v1.0.1\n");
    assert_eq!(fs::read_to_string(&file).unwrap(), "v1.0.1\n");
}

#[test]
fn json_format() {
    let sandbox = tagged("v1.4.0");
    sandbox.git(&["checkout", "--quiet", "-b", "feature/search"]);
    sandbox
        .commit("feat(search): add index")
        .commit("docs: describe search");
    let hash = sandbox.git(&["rev-parse", "HEAD~1"]).trim().to_owned();
    let output = sandbox
        .versinc()
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "previous_version": "1.4.0",
            "next_version": "1.5.0-dev.2",
            "bump": "minor",
            "branch": "feature/search",
            "prerelease_label": "dev",
            "config_files": [],
            "commits": [
                {"hash": hash, "subject": "feat(search): add index", "bump": "minor"}
            ]
        })
    );
}

#[test]
fn json_format_without_tags() {
    let sandbox = Sandbox::new();
    sandbox.commit("feat: a");
    let output = sandbox
        .versinc()
        .args(["--format", "json"])
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["previous_version"], serde_json::Value::Null);
    assert_eq!(json["next_version"], "0.1.0");
    assert_eq!(json["bump"], "none");
    assert_eq!(json["branch"], "main");
}

// Repository state

#[test]
fn shallow_clone_is_rejected() {
    let sandbox = tagged("v1.0.0");
    sandbox.commit("fix: a").commit("fix: b");
    let url = format!("file://{}", sandbox.repo.display());
    let clone = sandbox.root.join("shallow");
    sandbox.git_in(
        &sandbox.root,
        &[
            "clone",
            "--quiet",
            "--depth",
            "1",
            &url,
            clone.to_str().unwrap(),
        ],
    );
    sandbox
        .versinc_in(&clone)
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("shallow clone").and(contains("fetch-depth: 0")));
}

#[test]
fn outside_git_repo() {
    let sandbox = Sandbox::new();
    let outside = sandbox.root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    sandbox
        .versinc_in(&outside)
        .assert()
        .code(1)
        .stdout("")
        .stderr("error: not inside a git repository\n");
}

#[test]
fn missing_git_executable() {
    let sandbox = tagged("v1.0.0");
    sandbox
        .versinc()
        .env("PATH", sandbox.root.join("empty-bin"))
        .assert()
        .code(1)
        .stderr(contains("could not run `git`"));
}

// CLI surface

#[test]
fn help_and_version() {
    let sandbox = Sandbox::new();
    sandbox
        .versinc()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Usage: versinc").and(contains("init")));
    sandbox
        .versinc()
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("versinc {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn usage_errors_exit_with_2() {
    let sandbox = Sandbox::new();
    sandbox
        .versinc()
        .args(["--format", "yaml"])
        .assert()
        .code(2);
    sandbox
        .versinc()
        .args(["init", "--with-prefix"])
        .assert()
        .code(2);
}
