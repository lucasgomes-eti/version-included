//! Temporary git repositories for driving the `versinc` binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A temporary directory holding a fake home and a git repository, with git
/// and versinc isolated from the user's configuration and environment.
pub struct Sandbox {
    _dir: TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
    pub repo: PathBuf,
}

impl Sandbox {
    /// A sandbox with an initialized repository on branch `main` and no commits.
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        // Canonicalize so paths match what `git rev-parse --show-toplevel` prints
        // (macOS temp dirs live behind a /var -> /private/var symlink).
        let root = dir.path().canonicalize().unwrap();
        let home = root.join("home");
        let repo = root.join("repo");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&repo).unwrap();
        let sandbox = Sandbox {
            _dir: dir,
            root,
            home,
            repo,
        };
        sandbox.git(&["init", "--quiet", "--initial-branch=main"]);
        sandbox
    }

    fn isolate(&self, cmd: &mut Command) {
        cmd.env("HOME", &self.home)
            .env("GIT_CONFIG_GLOBAL", self.home.join(".gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CEILING_DIRECTORIES", &self.root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("GITHUB_HEAD_REF")
            .env_remove("CI")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE");
    }

    /// Runs git in `dir`, panicking on failure, and returns its stdout.
    pub fn git_in(&self, dir: &Path, args: &[&str]) -> String {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(dir);
        self.isolate(&mut cmd);
        let output = cmd.output().unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn git(&self, args: &[&str]) -> String {
        self.git_in(&self.repo, args)
    }

    /// Creates an empty commit with `message` (which may contain a body).
    pub fn commit(&self, message: &str) -> &Self {
        self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
        self
    }

    pub fn tag(&self, name: &str) -> &Self {
        self.git(&["tag", name]);
        self
    }

    /// Writes a file relative to the sandbox root, creating parent directories.
    pub fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }

    pub fn global_config(&self) -> PathBuf {
        self.home.join(".config/version-included/config.toml")
    }

    /// The versinc binary, run in the repository.
    pub fn versinc(&self) -> assert_cmd::Command {
        self.versinc_in(&self.repo)
    }

    pub fn versinc_in(&self, dir: &Path) -> assert_cmd::Command {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("versinc"));
        cmd.current_dir(dir);
        self.isolate(&mut cmd);
        assert_cmd::Command::from_std(cmd)
    }

    /// Runs versinc expecting success and returns its stdout.
    pub fn version(&self) -> String {
        let output = self.versinc().output().unwrap();
        assert!(
            output.status.success(),
            "versinc failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}
