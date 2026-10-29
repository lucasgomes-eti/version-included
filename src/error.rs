use std::fmt;
use std::io;
use std::path::PathBuf;

/// Every failure `versinc` can report. `Display` produces the message printed
/// after `error: ` on stderr.
#[derive(Debug)]
pub enum Error {
    /// The `git` executable could not be started.
    GitNotFound,
    /// A git command exited unsuccessfully.
    Git { command: String, stderr: String },
    /// The working directory is not inside a git work tree.
    NotARepo,
    /// The repository is a shallow clone, so tags and history are incomplete.
    ShallowClone,
    /// A config file is malformed or holds an invalid value.
    Config { file: String, message: String },
    /// The file given with `--config` does not exist.
    ConfigNotFound(PathBuf),
    /// `init` would overwrite an existing config file.
    ConfigExists(PathBuf),
    /// Neither `XDG_CONFIG_HOME` nor `HOME` is set.
    NoGlobalConfigDir,
    /// Reading or writing a file failed.
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl Error {
    pub fn io(action: &'static str, path: impl Into<PathBuf>, source: io::Error) -> Self {
        Error::Io {
            action,
            path: path.into(),
            source,
        }
    }

    pub fn config(file: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Config {
            file: file.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::GitNotFound => write!(f, "could not run `git`; is it installed and on PATH?"),
            Error::Git { command, stderr } => {
                write!(f, "`{command}` failed")?;
                let stderr = stderr.trim();
                if !stderr.is_empty() {
                    write!(f, ": {stderr}")?;
                }
                Ok(())
            }
            Error::NotARepo => write!(f, "not inside a git repository"),
            Error::ShallowClone => write!(
                f,
                "this is a shallow clone, so tags and commit history are incomplete; \
                 fetch the full history (`git fetch --unshallow --tags`, or \
                 `fetch-depth: 0` with actions/checkout)"
            ),
            Error::Config { file, message } => write!(f, "invalid config file {file}: {message}"),
            Error::ConfigNotFound(path) => {
                write!(f, "config file {} does not exist", path.display())
            }
            Error::ConfigExists(path) => write!(
                f,
                "{} already exists; pass --force to overwrite it",
                path.display()
            ),
            Error::NoGlobalConfigDir => write!(
                f,
                "cannot locate the global config directory: neither XDG_CONFIG_HOME nor HOME is set"
            ),
            Error::Io {
                action,
                path,
                source,
            } => write!(f, "failed to {action} {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_error_includes_trimmed_stderr() {
        let err = Error::Git {
            command: "git log".into(),
            stderr: "fatal: bad revision\n".into(),
        };
        assert_eq!(err.to_string(), "`git log` failed: fatal: bad revision");
    }

    #[test]
    fn git_error_without_stderr() {
        let err = Error::Git {
            command: "git log".into(),
            stderr: " \n".into(),
        };
        assert_eq!(err.to_string(), "`git log` failed");
    }

    #[test]
    fn shallow_clone_suggests_fetch_depth() {
        assert!(Error::ShallowClone.to_string().contains("fetch-depth: 0"));
    }

    #[test]
    fn config_error_names_file() {
        let err = Error::config("versinc.toml", "key `x`: bad");
        assert_eq!(
            err.to_string(),
            "invalid config file versinc.toml: key `x`: bad"
        );
    }

    #[test]
    fn io_error_names_action_and_path_and_exposes_source() {
        let err = Error::io(
            "read",
            "/tmp/x",
            io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        );
        assert_eq!(err.to_string(), "failed to read /tmp/x: denied");
        assert!(std::error::Error::source(&err).is_some());
        assert!(std::error::Error::source(&Error::NotARepo).is_none());
    }

    #[test]
    fn remaining_messages() {
        assert_eq!(Error::NotARepo.to_string(), "not inside a git repository");
        assert!(Error::GitNotFound.to_string().contains("PATH"));
        assert!(
            Error::NoGlobalConfigDir
                .to_string()
                .contains("XDG_CONFIG_HOME")
        );
        assert_eq!(
            Error::ConfigNotFound("a.toml".into()).to_string(),
            "config file a.toml does not exist"
        );
        assert_eq!(
            Error::ConfigExists("versinc.toml".into()).to_string(),
            "versinc.toml already exists; pass --force to overwrite it"
        );
    }
}
