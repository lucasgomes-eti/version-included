//! `versinc update`: replaces the running binary with the latest release.
//!
//! Like git access, downloading and unpacking shell out to standard tools
//! (`curl`, `sha256sum`/`shasum`, `tar`) behind the [`Tools`] trait, so the
//! update adds no dependencies and the logic here is testable with fakes.

use std::cmp::Ordering;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use semver::Version;

use crate::env::{Env, Fs};
use crate::error::Error;

/// Where releases are published. `VERSINC_RELEASES_URL` overrides it, for
/// mirrors and tests; it must have the same layout as GitHub Releases.
pub const RELEASES_URL: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases");
pub const RELEASES_URL_VAR: &str = "VERSINC_RELEASES_URL";

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const BIN_NAME: &str = "versinc";
/// Directory next to the binary where the new version is unpacked, so the
/// final rename stays on one filesystem and replaces the binary atomically.
const STAGING_DIR: &str = ".versinc-update";

/// The download, checksum and unpacking operations an update needs.
pub trait Tools {
    /// Downloads `url` and returns its body as text.
    fn fetch_text(&self, url: &str) -> Result<String, Error>;
    /// Downloads `url` into the file `dest`.
    fn download(&self, url: &str, dest: &Path) -> Result<(), Error>;
    /// The hex SHA-256 digest of a file.
    fn sha256(&self, path: &Path) -> Result<String, Error>;
    /// Unpacks a `.tar.xz` archive into `dir`, dropping its top-level directory.
    fn extract(&self, archive: &Path, dir: &Path) -> Result<(), Error>;
}

/// What `versinc update` was asked to do, and about which binary.
pub struct Request<'a> {
    /// Only report whether a newer release exists.
    pub check: bool,
    pub current: &'a Version,
    /// `std::env::consts::OS` and `ARCH` of the running binary.
    pub os: &'static str,
    pub arch: &'static str,
}

/// The release target triple for an OS and architecture, matching the
/// archives dist publishes.
pub fn target_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-musl"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        _ => None,
    }
}

/// The version of this app in a release's `dist-manifest.json`.
pub fn parse_manifest(text: &str) -> Result<Version, String> {
    let manifest: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("not valid JSON ({e})"))?;
    let version = manifest["releases"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|release| release["app_name"] == APP_NAME)
        .and_then(|release| release["app_version"].as_str())
        .ok_or_else(|| format!("no {APP_NAME} release listed"))?;
    Version::parse(version).map_err(|e| format!("invalid version {version:?} ({e})"))
}

/// The digest in a `<hash>  <file>` checksum file, if it holds one.
pub fn parse_checksum(text: &str) -> Option<String> {
    let digest = text.split_whitespace().next()?;
    (digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| digest.to_ascii_lowercase())
}

/// Checks for a newer release and, unless only checking, installs it over
/// the running binary. Status messages go to `stderr`.
pub fn run(
    request: &Request,
    env: &dyn Env,
    fs: &dyn Fs,
    tools: &dyn Tools,
    stderr: &mut dyn Write,
) -> Result<(), Error> {
    let base = env
        .var(RELEASES_URL_VAR)
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| RELEASES_URL.to_owned());
    let base = base.trim_end_matches('/');

    let manifest_url = format!("{base}/latest/download/dist-manifest.json");
    let latest = parse_manifest(&tools.fetch_text(&manifest_url)?).map_err(|message| {
        Error::InvalidRelease {
            url: manifest_url.clone(),
            message,
        }
    })?;
    let current = request.current;

    // Status output is best effort; a closed stderr must not fail the update.
    match latest.cmp(current) {
        Ordering::Less => {
            let _ = writeln!(
                stderr,
                "versinc {current} is newer than the latest release ({latest})"
            );
            return Ok(());
        }
        Ordering::Equal => {
            let _ = writeln!(stderr, "versinc {current} is up to date");
            return Ok(());
        }
        Ordering::Greater if request.check => {
            let _ = writeln!(
                stderr,
                "versinc {latest} is available (installed: {current}); \
                 run `versinc update` to install it"
            );
            return Ok(());
        }
        Ordering::Greater => {}
    }

    let target = target_for(request.os, request.arch).ok_or(Error::UnsupportedPlatform {
        os: request.os,
        arch: request.arch,
    })?;
    let exe = env
        .current_exe()
        .map_err(|e| Error::io("locate", "the running versinc executable", e))?;
    let dir = exe.parent().unwrap_or(Path::new(".")).to_path_buf();
    let staging = staging_dir(&exe);

    // Leftovers of an interrupted update; usually there is nothing to remove.
    let _ = fs.remove_dir_all(&staging);
    fs.create_dir_all(&staging)
        .map_err(|e| replace_error(&dir, "create directory", &staging, e))?;
    let installed = install(base, &latest, target, &exe, &staging, fs, tools, stderr);
    let _ = fs.remove_dir_all(&staging);
    installed?;

    let _ = writeln!(
        stderr,
        "updated versinc {current} -> {latest} ({})",
        exe.display()
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn install(
    base: &str,
    version: &Version,
    target: &str,
    exe: &Path,
    staging: &Path,
    fs: &dyn Fs,
    tools: &dyn Tools,
    stderr: &mut dyn Write,
) -> Result<(), Error> {
    let archive_name = format!("{APP_NAME}-{target}.tar.xz");
    let url = format!("{base}/download/v{version}/{archive_name}");
    let _ = writeln!(stderr, "downloading {url}");
    let archive = staging.join(&archive_name);
    tools.download(&url, &archive)?;

    let checksum_url = format!("{url}.sha256");
    let expected =
        parse_checksum(&tools.fetch_text(&checksum_url)?).ok_or_else(|| Error::InvalidRelease {
            url: checksum_url.clone(),
            message: "no SHA-256 digest found".into(),
        })?;
    let actual = tools.sha256(&archive)?.to_ascii_lowercase();
    if actual != expected {
        return Err(Error::ChecksumMismatch {
            file: archive_name,
            expected,
            actual,
        });
    }

    tools.extract(&archive, staging)?;
    let new_binary = staging.join(BIN_NAME);
    fs.set_executable(&new_binary)
        .map_err(|e| Error::io("prepare", &new_binary, e))?;
    let dir = exe.parent().unwrap_or(Path::new("."));
    fs.rename(&new_binary, exe)
        .map_err(|e| replace_error(dir, "replace", exe, e))
}

/// Permission problems get the dedicated, actionable error.
fn replace_error(dir: &Path, action: &'static str, path: &Path, e: io::Error) -> Error {
    if e.kind() == io::ErrorKind::PermissionDenied {
        Error::NotWritable(dir.to_path_buf())
    } else {
        Error::io(action, path, e)
    }
}

/// Runs `curl`, `sha256sum`/`shasum` and `tar`.
pub struct SystemTools;

/// HTTPS only (TLS 1.2+), plus `file://` for local mirrors; follows the
/// redirects GitHub uses for release assets; fails on HTTP errors.
const CURL_ARGS: [&str; 10] = [
    "--proto",
    "=https,file",
    "--tlsv1.2",
    "--fail",
    "--silent",
    "--show-error",
    "--location",
    "--retry",
    "3",
    "--",
];

impl SystemTools {
    fn run(program: &'static str, cmd: &mut Command) -> Result<Output, Error> {
        let output = cmd.output().map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => Error::ToolNotFound(program),
            _ => Error::Command {
                command: command_line(cmd),
                stderr: e.to_string(),
            },
        })?;
        if !output.status.success() {
            return Err(Error::Command {
                command: command_line(cmd),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(output)
    }

    fn digest(program: &'static str, cmd: &mut Command) -> Result<String, Error> {
        let output = Self::run(program, cmd)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_checksum(&stdout).ok_or_else(|| Error::Command {
            command: command_line(cmd),
            stderr: format!("unexpected output: {}", stdout.trim()),
        })
    }
}

impl Tools for SystemTools {
    fn fetch_text(&self, url: &str) -> Result<String, Error> {
        let output = Self::run("curl", Command::new("curl").args(CURL_ARGS).arg(url))?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn download(&self, url: &str, dest: &Path) -> Result<(), Error> {
        Self::run(
            "curl",
            Command::new("curl")
                .arg("--output")
                .arg(dest)
                .args(CURL_ARGS)
                .arg(url),
        )
        .map(drop)
    }

    fn sha256(&self, path: &Path) -> Result<String, Error> {
        // GNU coreutils on Linux; macOS ships `shasum` instead.
        match Self::digest("sha256sum", Command::new("sha256sum").arg(path)) {
            Err(Error::ToolNotFound(_)) => Self::digest(
                "shasum",
                Command::new("shasum").args(["-a", "256"]).arg(path),
            ),
            result => result,
        }
    }

    fn extract(&self, archive: &Path, dir: &Path) -> Result<(), Error> {
        Self::run(
            "tar",
            Command::new("tar")
                .arg("-xJf")
                .arg(archive)
                .arg("-C")
                .arg(dir)
                .arg("--strip-components=1"),
        )
        .map(drop)
    }
}

fn command_line(cmd: &Command) -> String {
    std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Path where `run` unpacks the new binary for an executable at `exe`.
pub fn staging_dir(exe: &Path) -> PathBuf {
    exe.parent().unwrap_or(Path::new(".")).join(STAGING_DIR)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::env::tests::{FakeEnv, FakeFs};
    use std::cell::RefCell;
    use std::collections::HashMap;

    const DIGEST: &str = "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03";
    const EXE: &str = "/home/u/.local/bin/versinc";
    const BASE: &str = "https://github.com/lucasgomes-eti/version-included/releases";
    const ARCHIVE_URL: &str = "https://github.com/lucasgomes-eti/version-included/releases\
                               /download/v0.4.0/version-included-x86_64-unknown-linux-musl.tar.xz";

    fn manifest(version: &str) -> String {
        format!(
            r#"{{"announcement_tag": "v{version}", "releases": [
                {{"app_name": "other", "app_version": "9.9.9"}},
                {{"app_name": "version-included", "app_version": "{version}"}}
            ]}}"#
        )
    }

    /// Serves `texts` and `downloads` by URL; downloads and extraction
    /// write into the given fake filesystem.
    pub(crate) struct FakeTools<'a> {
        pub(crate) fs: &'a FakeFs,
        pub(crate) texts: HashMap<String, String>,
        pub(crate) downloads: HashMap<String, String>,
        pub(crate) digest: String,
        pub(crate) fetched: RefCell<Vec<String>>,
    }

    impl<'a> FakeTools<'a> {
        pub(crate) fn new(fs: &'a FakeFs) -> Self {
            FakeTools {
                fs,
                texts: HashMap::new(),
                downloads: HashMap::new(),
                digest: DIGEST.into(),
                fetched: RefCell::default(),
            }
        }

        /// A release `latest` with a matching archive for linux x86_64.
        fn release(fs: &'a FakeFs, latest: &str) -> Self {
            let mut tools = FakeTools::new(fs);
            tools.texts.insert(
                format!("{BASE}/latest/download/dist-manifest.json"),
                manifest(latest),
            );
            tools.downloads.insert(ARCHIVE_URL.into(), "archive".into());
            tools.texts.insert(
                format!("{ARCHIVE_URL}.sha256"),
                format!("{DIGEST} *archive\n"),
            );
            tools
        }
    }

    fn missing(url: &str) -> Error {
        Error::Command {
            command: format!("curl {url}"),
            stderr: "curl: (22) The requested URL returned error: 404".into(),
        }
    }

    impl Tools for FakeTools<'_> {
        fn fetch_text(&self, url: &str) -> Result<String, Error> {
            self.fetched.borrow_mut().push(url.to_owned());
            self.texts.get(url).cloned().ok_or_else(|| missing(url))
        }

        fn download(&self, url: &str, dest: &Path) -> Result<(), Error> {
            self.fetched.borrow_mut().push(url.to_owned());
            let body = self.downloads.get(url).ok_or_else(|| missing(url))?;
            self.fs
                .write(dest, body, true)
                .map_err(|e| Error::io("write", dest, e))
        }

        fn sha256(&self, _path: &Path) -> Result<String, Error> {
            Ok(self.digest.clone())
        }

        fn extract(&self, _archive: &Path, dir: &Path) -> Result<(), Error> {
            let binary = dir.join(BIN_NAME);
            self.fs
                .write(&binary, "new versinc", true)
                .map_err(|e| Error::io("write", binary, e))
        }
    }

    struct Run {
        result: Result<(), Error>,
        stderr: String,
    }

    fn update(check: bool, current: &str, env: &FakeEnv, fs: &FakeFs, tools: &FakeTools) -> Run {
        let current = Version::parse(current).unwrap();
        let request = Request {
            check,
            current: &current,
            os: "linux",
            arch: "x86_64",
        };
        let mut stderr = Vec::new();
        let result = run(&request, env, fs, tools, &mut stderr);
        Run {
            result,
            stderr: String::from_utf8(stderr).unwrap(),
        }
    }

    fn installed() -> (FakeEnv, FakeFs) {
        (
            FakeEnv::new(&[]).with_exe(EXE),
            FakeFs::with_files(&[(EXE, "old versinc")]),
        )
    }

    #[test]
    fn targets_match_published_archives() {
        assert_eq!(
            target_for("linux", "x86_64"),
            Some("x86_64-unknown-linux-musl")
        );
        assert_eq!(
            target_for("linux", "aarch64"),
            Some("aarch64-unknown-linux-musl")
        );
        assert_eq!(target_for("macos", "x86_64"), Some("x86_64-apple-darwin"));
        assert_eq!(target_for("macos", "aarch64"), Some("aarch64-apple-darwin"));
        assert_eq!(target_for("windows", "x86_64"), None);
        assert_eq!(target_for("linux", "riscv64"), None);
    }

    #[test]
    fn parses_manifest_version_for_this_app() {
        assert_eq!(
            parse_manifest(&manifest("0.4.0")),
            Ok(Version::new(0, 4, 0))
        );
    }

    #[test]
    fn rejects_bad_manifests() {
        assert!(
            parse_manifest("<html>")
                .unwrap_err()
                .starts_with("not valid JSON")
        );
        assert_eq!(
            parse_manifest(r#"{"releases": []}"#).unwrap_err(),
            "no version-included release listed"
        );
        assert_eq!(
            parse_manifest("{}").unwrap_err(),
            "no version-included release listed"
        );
        assert!(
            parse_manifest(
                r#"{"releases": [{"app_name": "version-included", "app_version": "1.x"}]}"#
            )
            .unwrap_err()
            .starts_with("invalid version \"1.x\"")
        );
    }

    #[test]
    fn parses_checksum_files() {
        assert_eq!(
            parse_checksum(&format!("{DIGEST} *file.tar.xz\n\n")),
            Some(DIGEST.into())
        );
        assert_eq!(
            parse_checksum(&format!("{}  file", DIGEST.to_uppercase())),
            Some(DIGEST.into())
        );
        assert_eq!(parse_checksum(""), None);
        assert_eq!(parse_checksum("abc123 file"), None);
        assert_eq!(parse_checksum(&format!("{}zz file", &DIGEST[..62])), None);
    }

    #[test]
    fn installs_newer_release_over_the_running_binary() {
        let (env, fs) = installed();
        let tools = FakeTools::release(&fs, "0.4.0");
        let run = update(false, "0.3.0", &env, &fs, &tools);
        run.result.unwrap();
        assert_eq!(fs.file(EXE).as_deref(), Some("new versinc"));
        let staging = staging_dir(Path::new(EXE));
        assert_eq!(*fs.executables.borrow(), vec![staging.join(BIN_NAME)]);
        // The staging directory is cleaned before and after.
        assert_eq!(*fs.removed_dirs.borrow(), vec![staging.clone(), staging]);
        assert!(fs.file("/home/u/.local/bin/.versinc-update/version-included-x86_64-unknown-linux-musl.tar.xz").is_none());
        assert_eq!(
            run.stderr,
            format!("downloading {ARCHIVE_URL}\nupdated versinc 0.3.0 -> 0.4.0 ({EXE})\n")
        );
    }

    #[test]
    fn up_to_date() {
        let (env, fs) = installed();
        let tools = FakeTools::release(&fs, "0.3.0");
        let run = update(false, "0.3.0", &env, &fs, &tools);
        run.result.unwrap();
        assert_eq!(run.stderr, "versinc 0.3.0 is up to date\n");
        assert_eq!(fs.file(EXE).as_deref(), Some("old versinc"));
        assert_eq!(tools.fetched.borrow().len(), 1);
    }

    #[test]
    fn newer_than_latest_release() {
        let (env, fs) = installed();
        let tools = FakeTools::release(&fs, "0.3.0");
        let run = update(false, "0.4.0-dev.2", &env, &fs, &tools);
        run.result.unwrap();
        assert_eq!(
            run.stderr,
            "versinc 0.4.0-dev.2 is newer than the latest release (0.3.0)\n"
        );
        assert_eq!(fs.file(EXE).as_deref(), Some("old versinc"));
    }

    #[test]
    fn check_only_reports() {
        let (env, fs) = installed();
        let tools = FakeTools::release(&fs, "0.4.0");
        let run = update(true, "0.3.0", &env, &fs, &tools);
        run.result.unwrap();
        assert_eq!(
            run.stderr,
            "versinc 0.4.0 is available (installed: 0.3.0); run `versinc update` to install it\n"
        );
        assert_eq!(fs.file(EXE).as_deref(), Some("old versinc"));
        assert!(fs.dirs.borrow().is_empty());
    }

    #[test]
    fn releases_url_can_be_overridden() {
        let (_, fs) = installed();
        let env = FakeEnv::new(&[(RELEASES_URL_VAR, "file:///mirror/releases/")]).with_exe(EXE);
        let mut tools = FakeTools::new(&fs);
        tools.texts.insert(
            "file:///mirror/releases/latest/download/dist-manifest.json".into(),
            manifest("0.3.0"),
        );
        update(true, "0.3.0", &env, &fs, &tools).result.unwrap();
        // An empty override falls back to GitHub.
        let env = FakeEnv::new(&[(RELEASES_URL_VAR, "")]).with_exe(EXE);
        let tools = FakeTools::release(&fs, "0.3.0");
        update(true, "0.3.0", &env, &fs, &tools).result.unwrap();
    }

    #[test]
    fn missing_manifest_is_reported() {
        let (env, fs) = installed();
        let tools = FakeTools::new(&fs);
        let message = update(false, "0.3.0", &env, &fs, &tools)
            .result
            .unwrap_err()
            .to_string();
        assert!(message.contains("dist-manifest.json"), "{message}");
        assert!(message.contains("404"), "{message}");
    }

    #[test]
    fn invalid_manifest_names_the_url() {
        let (env, fs) = installed();
        let mut tools = FakeTools::new(&fs);
        tools.texts.insert(
            format!("{BASE}/latest/download/dist-manifest.json"),
            "{}".into(),
        );
        let message = update(false, "0.3.0", &env, &fs, &tools)
            .result
            .unwrap_err()
            .to_string();
        assert_eq!(
            message,
            format!(
                "unexpected release asset {BASE}/latest/download/dist-manifest.json: \
                 no version-included release listed"
            )
        );
    }

    #[test]
    fn unsupported_platform() {
        let (env, fs) = installed();
        let tools = FakeTools::release(&fs, "0.4.0");
        let current = Version::new(0, 3, 0);
        let request = Request {
            check: false,
            current: &current,
            os: "windows",
            arch: "x86_64",
        };
        let result = run(&request, &env, &fs, &tools, &mut Vec::new());
        assert!(matches!(
            result,
            Err(Error::UnsupportedPlatform {
                os: "windows",
                arch: "x86_64"
            })
        ));
        // --check still works: it needs no binary.
        let request = Request {
            check: true,
            ..request
        };
        run(&request, &env, &fs, &tools, &mut Vec::new()).unwrap();
    }

    #[test]
    fn unknown_executable_location() {
        let fs = FakeFs::default();
        let tools = FakeTools::release(&fs, "0.4.0");
        let message = update(false, "0.3.0", &FakeEnv::new(&[]), &fs, &tools)
            .result
            .unwrap_err()
            .to_string();
        assert!(
            message.starts_with("failed to locate the running versinc executable"),
            "{message}"
        );
    }

    #[test]
    fn read_only_install_directory() {
        let (env, mut fs) = installed();
        fs.read_only_dirs = vec![PathBuf::from("/home/u/.local/bin")];
        let tools = FakeTools::release(&fs, "0.4.0");
        let result = update(false, "0.3.0", &env, &fs, &tools).result;
        assert!(
            matches!(&result, Err(Error::NotWritable(dir)) if dir == Path::new("/home/u/.local/bin")),
            "{result:?}"
        );
        assert_eq!(fs.file(EXE).as_deref(), Some("old versinc"));
    }

    #[test]
    fn checksum_mismatch_keeps_the_old_binary() {
        let (env, fs) = installed();
        let mut tools = FakeTools::release(&fs, "0.4.0");
        tools.digest = "0".repeat(64);
        let result = update(false, "0.3.0", &env, &fs, &tools).result;
        assert!(
            matches!(result, Err(Error::ChecksumMismatch { .. })),
            "{result:?}"
        );
        assert_eq!(fs.file(EXE).as_deref(), Some("old versinc"));
        // The staging directory is removed even on failure.
        assert_eq!(fs.removed_dirs.borrow().len(), 2);
    }

    #[test]
    fn malformed_checksum_file() {
        let (env, fs) = installed();
        let mut tools = FakeTools::release(&fs, "0.4.0");
        tools
            .texts
            .insert(format!("{ARCHIVE_URL}.sha256"), "Not Found".into());
        let message = update(false, "0.3.0", &env, &fs, &tools)
            .result
            .unwrap_err()
            .to_string();
        assert_eq!(
            message,
            format!("unexpected release asset {ARCHIVE_URL}.sha256: no SHA-256 digest found")
        );
    }

    #[test]
    fn missing_archive() {
        let (env, fs) = installed();
        let mut tools = FakeTools::release(&fs, "0.4.0");
        tools.downloads.clear();
        let message = update(false, "0.3.0", &env, &fs, &tools)
            .result
            .unwrap_err()
            .to_string();
        assert!(message.contains("404"), "{message}");
        assert_eq!(fs.file(EXE).as_deref(), Some("old versinc"));
    }

    #[test]
    fn replace_error_maps_permissions() {
        let dir = Path::new("/bin");
        let denied = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        assert!(matches!(
            replace_error(dir, "replace", Path::new("/bin/versinc"), denied),
            Error::NotWritable(_)
        ));
        let busy = io::Error::other("busy");
        assert!(matches!(
            replace_error(dir, "replace", Path::new("/bin/versinc"), busy),
            Error::Io {
                action: "replace",
                ..
            }
        ));
    }

    // SystemTools against local files (curl supports file:// URLs).

    fn file_url(path: &Path) -> String {
        format!("file://{}", path.display())
    }

    #[test]
    fn system_tools_fetch_and_download() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("hello.txt");
        std::fs::write(&source, "hello\n").unwrap();

        assert_eq!(
            SystemTools.fetch_text(&file_url(&source)).unwrap(),
            "hello\n"
        );
        let dest = dir.path().join("copy.txt");
        SystemTools.download(&file_url(&source), &dest).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hello\n");
        assert_eq!(SystemTools.sha256(&dest).unwrap(), DIGEST);

        let missing = file_url(&dir.path().join("missing"));
        let err = SystemTools.fetch_text(&missing).unwrap_err().to_string();
        assert!(err.starts_with("`curl --proto"), "{err}");
        assert!(SystemTools.download(&missing, &dest).is_err());
        assert!(SystemTools.sha256(&dir.path().join("missing")).is_err());
    }

    #[test]
    fn system_tools_extract_tar_xz() {
        let dir = tempfile::tempdir().unwrap();
        let content = dir.path().join("version-included-x/versinc");
        std::fs::create_dir_all(content.parent().unwrap()).unwrap();
        std::fs::write(&content, "binary").unwrap();
        let archive = dir.path().join("a.tar.xz");
        let status = Command::new("tar")
            .arg("-cJf")
            .arg(&archive)
            .arg("-C")
            .arg(dir.path())
            .arg("version-included-x")
            .status()
            .unwrap();
        assert!(status.success());

        let out = dir.path().join("out");
        std::fs::create_dir(&out).unwrap();
        SystemTools.extract(&archive, &out).unwrap();
        assert_eq!(
            std::fs::read_to_string(out.join("versinc")).unwrap(),
            "binary"
        );

        let err = SystemTools
            .extract(&dir.path().join("none.tar.xz"), &out)
            .unwrap_err();
        assert!(err.to_string().starts_with("`tar -xJf"), "{err}");
    }

    #[test]
    fn command_line_joins_program_and_args() {
        let mut cmd = Command::new("tar");
        cmd.args(["-xJf", "a b.tar.xz"]);
        assert_eq!(command_line(&cmd), "tar -xJf a b.tar.xz");
    }
}
