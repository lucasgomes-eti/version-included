use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::branch;
use crate::cli::{CalculateArgs, Cli, Command, Format, InitArgs, UpdateArgs};
use crate::config::{self, Config, Source};
use crate::env::{Env, Fs};
use crate::error::Error;
use crate::git::Git;
use crate::output;
use crate::tags;
use crate::update::{self, Tools};
use crate::version;

/// The outside world: git, the process environment, the filesystem, and the
/// download tools `update` uses.
pub struct Deps<'a> {
    pub git: &'a dyn Git,
    pub env: &'a dyn Env,
    pub fs: &'a dyn Fs,
    pub tools: &'a dyn Tools,
}

/// Runs the parsed command line. Results go to `stdout`; warnings and
/// status messages go to `stderr`. Errors are returned for the caller to print.
pub fn run(
    cli: &Cli,
    deps: &Deps,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), Error> {
    match &cli.command {
        Some(Command::Init(args)) => init(args, deps, stderr),
        Some(Command::Update(args)) => self_update(args, deps, stderr),
        None => calculate(&cli.calculate, deps, stdout, stderr),
    }
}

fn calculate(
    args: &CalculateArgs,
    deps: &Deps,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), Error> {
    let root = deps.git.toplevel()?;
    if deps.git.is_shallow()? {
        return Err(Error::ShallowClone);
    }
    let (config, config_files) = load_config(deps, &root, args.config.as_deref())?;

    let (tag, commits, branch) = if deps.git.has_head()? {
        let tag = tags::latest(&deps.git.tags_merged_into_head()?, &config.tag_prefix);
        let commits = deps
            .git
            .commits_since(tag.as_ref().map(|t| t.name.as_str()))?;
        let head = deps.git.abbrev_ref_head()?;
        let branch = branch::resolve(Some(&head), deps.env.var("GITHUB_HEAD_REF").as_deref());
        if branch.is_none() {
            // Diagnostics are best effort; a closed stderr must not fail the run.
            let _ = writeln!(
                stderr,
                "warning: cannot determine the current branch (HEAD is detached and \
                 GITHUB_HEAD_REF is not set); treating this as a release build"
            );
        }
        (tag, commits, branch)
    } else {
        // A repository without commits has no tags or history yet.
        (None, Vec::new(), None)
    };

    let outcome = version::calculate(
        tag.as_ref().map(|t| &t.version),
        &commits,
        &config,
        branch.as_deref(),
    );
    let prefix = args.with_prefix.then_some(config.tag_prefix.as_str());
    let version = output::version_string(&outcome.next, prefix);

    if let Some(path) = &args.output {
        deps.fs
            .write(path, &format!("{version}\n"), true)
            .map_err(|e| Error::io("write", path, e))?;
    }
    let text = match args.format {
        Format::Text => version,
        Format::Json => output::json(&outcome, branch.as_deref(), &config_files, prefix),
    };
    writeln!(stdout, "{text}").map_err(|e| Error::io("write", "standard output", e))
}

/// Loads the global, project and `--config` files that exist and merges them
/// over the built-in defaults. Returns the config and the loaded file paths.
fn load_config(
    deps: &Deps,
    repo_root: &Path,
    explicit: Option<&Path>,
) -> Result<(Config, Vec<String>), Error> {
    let mut candidates: Vec<(PathBuf, bool)> = Vec::new();
    if let Some(global) = config::global_path(deps.env) {
        candidates.push((global, false));
    }
    candidates.push((config::project_path(repo_root), false));
    if let Some(explicit) = explicit {
        candidates.push((explicit.to_path_buf(), true));
    }

    let mut sources = Vec::new();
    for (path, required) in candidates {
        match deps.fs.read_to_string(&path) {
            Ok(text) => sources.push(Source::new(path.display().to_string(), text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound && !required => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(Error::ConfigNotFound(path));
            }
            Err(e) => return Err(Error::io("read", path, e)),
        }
    }
    let config = Config::merge(&sources)?;
    Ok((config, sources.into_iter().map(|s| s.name).collect()))
}

fn self_update(args: &UpdateArgs, deps: &Deps, stderr: &mut dyn Write) -> Result<(), Error> {
    let current =
        semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("package version is SemVer");
    let request = update::Request {
        check: args.check,
        current: &current,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    };
    update::run(&request, deps.env, deps.fs, deps.tools, stderr)
}

fn init(args: &InitArgs, deps: &Deps, stderr: &mut dyn Write) -> Result<(), Error> {
    let path = if args.global {
        let path = config::global_path(deps.env).ok_or(Error::NoGlobalConfigDir)?;
        if let Some(dir) = path.parent() {
            deps.fs
                .create_dir_all(dir)
                .map_err(|e| Error::io("create directory", dir, e))?;
        }
        path
    } else {
        config::project_path(&deps.git.toplevel()?)
    };

    match deps.fs.write(&path, config::DEFAULT_TOML, args.force) {
        Ok(()) => {
            let _ = writeln!(stderr, "wrote {}", path.display());
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(Error::ConfigExists(path)),
        Err(e) => Err(Error::io("write", path, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit::Commit;
    use crate::env::tests::{FakeEnv, FakeFs};
    use crate::update::tests::FakeTools;
    use clap::Parser;
    use std::cell::RefCell;

    struct FakeGit {
        toplevel: Option<&'static str>,
        shallow: bool,
        head: bool,
        tags: Vec<&'static str>,
        commits: Vec<Commit>,
        abbrev_ref: &'static str,
        /// The `tag` argument of each `commits_since` call.
        since: RefCell<Vec<Option<String>>>,
    }

    impl Default for FakeGit {
        fn default() -> Self {
            FakeGit {
                toplevel: Some("/repo"),
                shallow: false,
                head: true,
                tags: vec![],
                commits: vec![],
                abbrev_ref: "main",
                since: RefCell::default(),
            }
        }
    }

    impl Git for FakeGit {
        fn toplevel(&self) -> Result<PathBuf, Error> {
            self.toplevel.map(PathBuf::from).ok_or(Error::NotARepo)
        }
        fn is_shallow(&self) -> Result<bool, Error> {
            Ok(self.shallow)
        }
        fn has_head(&self) -> Result<bool, Error> {
            Ok(self.head)
        }
        fn tags_merged_into_head(&self) -> Result<Vec<String>, Error> {
            Ok(self.tags.iter().map(|t| t.to_string()).collect())
        }
        fn commits_since(&self, tag: Option<&str>) -> Result<Vec<Commit>, Error> {
            self.since.borrow_mut().push(tag.map(str::to_owned));
            Ok(self.commits.clone())
        }
        fn abbrev_ref_head(&self) -> Result<String, Error> {
            Ok(self.abbrev_ref.to_owned())
        }
    }

    fn commits(messages: &[&str]) -> Vec<Commit> {
        messages
            .iter()
            .enumerate()
            .map(|(i, m)| Commit::new(format!("h{i}"), *m))
            .collect()
    }

    struct Run {
        result: Result<(), Error>,
        stdout: String,
        stderr: String,
    }

    fn run_with(args: &[&str], git: &FakeGit, env: &FakeEnv, fs: &FakeFs) -> Run {
        run_with_tools(args, git, env, fs, &FakeTools::new(fs))
    }

    fn run_with_tools(
        args: &[&str],
        git: &FakeGit,
        env: &FakeEnv,
        fs: &FakeFs,
        tools: &FakeTools,
    ) -> Run {
        let cli =
            Cli::try_parse_from(std::iter::once("versinc").chain(args.iter().copied())).unwrap();
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let deps = Deps {
            git,
            env,
            fs,
            tools,
        };
        let result = run(&cli, &deps, &mut stdout, &mut stderr);
        Run {
            result,
            stdout: String::from_utf8(stdout).unwrap(),
            stderr: String::from_utf8(stderr).unwrap(),
        }
    }

    fn home() -> FakeEnv {
        FakeEnv::new(&[("HOME", "/home/u")])
    }

    const GLOBAL: &str = "/home/u/.config/version-included/config.toml";

    #[test]
    fn prints_next_version() {
        let git = FakeGit {
            tags: vec!["v1.2.3", "v1.0.0", "other"],
            commits: commits(&["feat: a", "fix: b"]),
            ..FakeGit::default()
        };
        let run = run_with(&[], &git, &home(), &FakeFs::default());
        run.result.unwrap();
        assert_eq!(run.stdout, "1.3.0\n");
        assert_eq!(run.stderr, "");
        assert_eq!(*git.since.borrow(), vec![Some("v1.2.3".to_owned())]);
    }

    #[test]
    fn without_tags_reads_all_commits() {
        let git = FakeGit {
            commits: commits(&["feat: a"]),
            ..FakeGit::default()
        };
        let run = run_with(&[], &git, &home(), &FakeFs::default());
        assert_eq!(run.stdout, "0.1.0\n");
        assert_eq!(*git.since.borrow(), vec![None]);
    }

    #[test]
    fn with_prefix() {
        let git = FakeGit {
            tags: vec!["v1.2.3"],
            commits: commits(&["fix: a"]),
            ..FakeGit::default()
        };
        let run = run_with(&["--with-prefix"], &git, &home(), &FakeFs::default());
        assert_eq!(run.stdout, "v1.2.4\n");
    }

    #[test]
    fn writes_output_file() {
        let git = FakeGit {
            tags: vec!["v1.2.3"],
            commits: commits(&["fix: a"]),
            ..FakeGit::default()
        };
        let fs = FakeFs::default();
        let run = run_with(
            &["--output", "/out/version.txt", "--format", "json"],
            &git,
            &home(),
            &fs,
        );
        run.result.unwrap();
        assert_eq!(fs.file("/out/version.txt").as_deref(), Some("1.2.4\n"));
        assert!(run.stdout.contains("\"next_version\": \"1.2.4\""));
    }

    #[test]
    fn output_file_write_failure() {
        let fs = FakeFs {
            read_only_dirs: vec![PathBuf::from("/ro")],
            ..FakeFs::default()
        };
        let run = run_with(
            &["--output", "/ro/v.txt"],
            &FakeGit::default(),
            &home(),
            &fs,
        );
        let message = run.result.unwrap_err().to_string();
        assert!(
            message.starts_with("failed to write /ro/v.txt"),
            "{message}"
        );
        assert_eq!(run.stdout, "");
    }

    #[test]
    fn json_lists_loaded_config_files_in_order() {
        let fs = FakeFs::with_files(&[
            (GLOBAL, "bump_unmatched_commits = true"),
            ("/repo/versinc.toml", "tag_prefix = \"\""),
            ("/elsewhere/ci.toml", "[rules]\nminor = []"),
        ]);
        let git = FakeGit {
            tags: vec!["2.0.0", "v9.0.0"],
            commits: commits(&["feat: a", "docs: b"]),
            abbrev_ref: "feature/x",
            ..FakeGit::default()
        };
        let run = run_with(
            &["--config", "/elsewhere/ci.toml", "--format", "json"],
            &git,
            &home(),
            &fs,
        );
        run.result.unwrap();
        let value: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
        assert_eq!(
            value["config_files"],
            serde_json::json!([GLOBAL, "/repo/versinc.toml", "/elsewhere/ci.toml"])
        );
        // feat is no longer a rule, so the two unmatched commits bump the patch.
        assert_eq!(value["previous_version"], "2.0.0");
        assert_eq!(value["next_version"], "2.0.2-dev.2");
        assert_eq!(value["bump"], "patch");
        assert_eq!(value["branch"], "feature/x");
        assert_eq!(value["prerelease_label"], "dev");
        assert_eq!(value["commits"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn missing_explicit_config_is_an_error() {
        let run = run_with(
            &["--config", "/nope.toml"],
            &FakeGit::default(),
            &home(),
            &FakeFs::default(),
        );
        assert!(
            matches!(run.result, Err(Error::ConfigNotFound(p)) if p == Path::new("/nope.toml"))
        );
    }

    #[test]
    fn unreadable_config_is_an_error() {
        let fs = FakeFs {
            unreadable: vec![PathBuf::from("/repo/versinc.toml")],
            ..FakeFs::default()
        };
        let run = run_with(&[], &FakeGit::default(), &home(), &fs);
        let message = run.result.unwrap_err().to_string();
        assert!(
            message.starts_with("failed to read /repo/versinc.toml"),
            "{message}"
        );
    }

    #[test]
    fn invalid_config_names_the_file() {
        let fs = FakeFs::with_files(&[("/repo/versinc.toml", "nope = 1")]);
        let run = run_with(&[], &FakeGit::default(), &home(), &fs);
        let message = run.result.unwrap_err().to_string();
        assert!(
            message.starts_with("invalid config file /repo/versinc.toml"),
            "{message}"
        );
    }

    #[test]
    fn works_without_home() {
        let git = FakeGit {
            tags: vec!["v1.0.0"],
            ..FakeGit::default()
        };
        let run = run_with(&[], &git, &FakeEnv::new(&[]), &FakeFs::default());
        assert_eq!(run.stdout, "1.0.0\n");
    }

    #[test]
    fn not_a_repo() {
        let git = FakeGit {
            toplevel: None,
            ..FakeGit::default()
        };
        let run = run_with(&[], &git, &home(), &FakeFs::default());
        assert!(matches!(run.result, Err(Error::NotARepo)));
    }

    #[test]
    fn shallow_clone_is_an_error() {
        let git = FakeGit {
            shallow: true,
            ..FakeGit::default()
        };
        let run = run_with(&[], &git, &home(), &FakeFs::default());
        assert!(matches!(run.result, Err(Error::ShallowClone)));
    }

    #[test]
    fn repository_without_commits_prints_initial_version() {
        let git = FakeGit {
            head: false,
            abbrev_ref: "HEAD",
            ..FakeGit::default()
        };
        let run = run_with(&[], &git, &home(), &FakeFs::default());
        assert_eq!(run.stdout, "0.1.0\n");
        assert_eq!(run.stderr, "");
        assert!(git.since.borrow().is_empty());
    }

    #[test]
    fn detached_head_without_github_head_ref_warns_and_releases() {
        let git = FakeGit {
            tags: vec!["v1.0.0"],
            commits: commits(&["feat: a"]),
            abbrev_ref: "HEAD",
            ..FakeGit::default()
        };
        let run = run_with(&[], &git, &home(), &FakeFs::default());
        assert_eq!(run.stdout, "1.1.0\n");
        assert!(
            run.stderr
                .starts_with("warning: cannot determine the current branch")
        );
    }

    #[test]
    fn detached_head_uses_github_head_ref() {
        let git = FakeGit {
            tags: vec!["v1.0.0"],
            commits: commits(&["feat: a"]),
            abbrev_ref: "HEAD",
            ..FakeGit::default()
        };
        let env = FakeEnv::new(&[("HOME", "/home/u"), ("GITHUB_HEAD_REF", "feature/pr")]);
        let run = run_with(&[], &git, &env, &FakeFs::default());
        assert_eq!(run.stdout, "1.1.0-dev.1\n");
        assert_eq!(run.stderr, "");
    }

    #[test]
    fn init_writes_project_config_at_repo_root() {
        let fs = FakeFs::default();
        let run = run_with(&["init"], &FakeGit::default(), &home(), &fs);
        run.result.unwrap();
        assert_eq!(
            fs.file("/repo/versinc.toml").as_deref(),
            Some(config::DEFAULT_TOML)
        );
        assert_eq!(run.stderr, "wrote /repo/versinc.toml\n");
        assert_eq!(run.stdout, "");
    }

    #[test]
    fn init_refuses_to_overwrite_without_force() {
        let fs = FakeFs::with_files(&[("/repo/versinc.toml", "mine")]);
        let run = run_with(&["init"], &FakeGit::default(), &home(), &fs);
        assert!(matches!(run.result, Err(Error::ConfigExists(_))));
        assert_eq!(fs.file("/repo/versinc.toml").as_deref(), Some("mine"));

        run_with(&["init", "--force"], &FakeGit::default(), &home(), &fs)
            .result
            .unwrap();
        assert_eq!(
            fs.file("/repo/versinc.toml").as_deref(),
            Some(config::DEFAULT_TOML)
        );
    }

    #[test]
    fn init_outside_repo() {
        let git = FakeGit {
            toplevel: None,
            ..FakeGit::default()
        };
        let run = run_with(&["init"], &git, &home(), &FakeFs::default());
        assert!(matches!(run.result, Err(Error::NotARepo)));
    }

    #[test]
    fn init_global_creates_directories() {
        let git = FakeGit {
            toplevel: None,
            ..FakeGit::default()
        };
        let fs = FakeFs::default();
        let run = run_with(&["init", "--global"], &git, &home(), &fs);
        run.result.unwrap();
        assert_eq!(fs.file(GLOBAL).as_deref(), Some(config::DEFAULT_TOML));
        assert_eq!(
            *fs.dirs.borrow(),
            vec![PathBuf::from("/home/u/.config/version-included")]
        );
    }

    #[test]
    fn init_global_without_home() {
        let run = run_with(
            &["init", "--global"],
            &FakeGit::default(),
            &FakeEnv::new(&[]),
            &FakeFs::default(),
        );
        assert!(matches!(run.result, Err(Error::NoGlobalConfigDir)));
    }

    #[test]
    fn init_reports_directory_and_write_failures() {
        let fs = FakeFs {
            read_only_dirs: vec![PathBuf::from("/home/u")],
            ..FakeFs::default()
        };
        let message = run_with(&["init", "--global"], &FakeGit::default(), &home(), &fs)
            .result
            .unwrap_err()
            .to_string();
        assert!(
            message.starts_with("failed to create directory /home/u/.config/version-included"),
            "{message}"
        );

        let fs = FakeFs {
            read_only_dirs: vec![PathBuf::from("/repo")],
            ..FakeFs::default()
        };
        let message = run_with(&["init"], &FakeGit::default(), &home(), &fs)
            .result
            .unwrap_err()
            .to_string();
        assert!(
            message.starts_with("failed to write /repo/versinc.toml"),
            "{message}"
        );
    }

    #[test]
    fn update_checks_the_running_version() {
        let fs = FakeFs::default();
        let mut tools = FakeTools::new(&fs);
        tools.texts.insert(
            format!(
                "{}/latest/download/dist-manifest.json",
                update::RELEASES_URL
            ),
            format!(
                r#"{{"releases": [{{"app_name": "version-included", "app_version": "{}"}}]}}"#,
                env!("CARGO_PKG_VERSION")
            ),
        );
        let git = FakeGit {
            toplevel: None,
            ..FakeGit::default()
        };
        let run = run_with_tools(&["update", "--check"], &git, &home(), &fs, &tools);
        run.result.unwrap();
        assert_eq!(
            run.stderr,
            format!("versinc {} is up to date\n", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(run.stdout, "");
    }
}
