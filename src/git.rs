use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};

use crate::commit::Commit;
use crate::error::Error;

/// The repository queries `versinc` needs. Implemented by [`SystemGit`] for
/// real use and by fakes in tests.
pub trait Git {
    /// Root of the work tree containing the current directory.
    fn toplevel(&self) -> Result<PathBuf, Error>;
    fn is_shallow(&self) -> Result<bool, Error>;
    /// Whether HEAD points at a commit (false in a repository with no commits).
    fn has_head(&self) -> Result<bool, Error>;
    /// Names of all tags reachable from HEAD.
    fn tags_merged_into_head(&self) -> Result<Vec<String>, Error>;
    /// Non-merge commits reachable from HEAD but not from `tag`, newest
    /// first; all non-merge commits when `tag` is `None`.
    fn commits_since(&self, tag: Option<&str>) -> Result<Vec<Commit>, Error>;
    /// Output of `git rev-parse --abbrev-ref HEAD`: the branch name, or
    /// `HEAD` when detached.
    fn abbrev_ref_head(&self) -> Result<String, Error>;
}

/// Runs the `git` executable in the current directory.
pub struct SystemGit;

/// Separates hash from message within a `git log` record.
const FIELD_SEP: char = '\x00';
/// Terminates each `git log` record.
const RECORD_SEP: char = '\x1e';
const LOG_FORMAT: &str = "--format=%H%x00%B%x1e";

impl SystemGit {
    fn run(&self, args: &[&str]) -> Result<Output, Error> {
        Command::new("git")
            .args(args)
            .output()
            .map_err(|e| match e.kind() {
                io::ErrorKind::NotFound => Error::GitNotFound,
                _ => Error::Git {
                    command: command_line(args),
                    stderr: e.to_string(),
                },
            })
    }

    /// Runs git and returns its stdout, failing if it exits unsuccessfully.
    fn stdout(&self, args: &[&str]) -> Result<String, Error> {
        let output = self.run(args)?;
        if !output.status.success() {
            return Err(Error::Git {
                command: command_line(args),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl Git for SystemGit {
    fn toplevel(&self) -> Result<PathBuf, Error> {
        let output = self.run(&["rev-parse", "--show-toplevel"])?;
        if !output.status.success() {
            return Err(Error::NotARepo);
        }
        let path = String::from_utf8_lossy(&output.stdout);
        Ok(PathBuf::from(path.trim_end_matches(['\n', '\r'])))
    }

    fn is_shallow(&self) -> Result<bool, Error> {
        Ok(self
            .stdout(&["rev-parse", "--is-shallow-repository"])?
            .trim()
            == "true")
    }

    fn has_head(&self) -> Result<bool, Error> {
        let output = self.run(&["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])?;
        Ok(output.status.success())
    }

    fn tags_merged_into_head(&self) -> Result<Vec<String>, Error> {
        Ok(self
            .stdout(&["tag", "--merged", "HEAD"])?
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect())
    }

    fn commits_since(&self, tag: Option<&str>) -> Result<Vec<Commit>, Error> {
        let range = match tag {
            Some(tag) => format!("refs/tags/{tag}..HEAD"),
            None => "HEAD".to_owned(),
        };
        let output = self.stdout(&[
            "log",
            "--no-merges",
            "--no-show-signature",
            LOG_FORMAT,
            &range,
            "--",
        ])?;
        Ok(parse_log(&output))
    }

    fn abbrev_ref_head(&self) -> Result<String, Error> {
        Ok(self
            .stdout(&["rev-parse", "--abbrev-ref", "HEAD"])?
            .trim()
            .to_owned())
    }
}

fn command_line(args: &[&str]) -> String {
    format!("git {}", args.join(" "))
}

/// Parses `git log` output written with [`LOG_FORMAT`].
pub fn parse_log(output: &str) -> Vec<Commit> {
    output
        .split(RECORD_SEP)
        .filter_map(|record| {
            let record = record.trim_start_matches(['\n', '\r']);
            let (hash, message) = record.split_once(FIELD_SEP)?;
            Some(Commit::new(hash.trim(), message.trim_end()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_records() {
        let output = "aaa\x00feat: one\n\nbody line\n\x1e\nbbb\x00fix: two\n\x1e\n";
        assert_eq!(
            parse_log(output),
            vec![
                Commit::new("aaa", "feat: one\n\nbody line"),
                Commit::new("bbb", "fix: two"),
            ]
        );
    }

    #[test]
    fn parses_empty_output() {
        assert!(parse_log("").is_empty());
        assert!(parse_log("\n").is_empty());
    }

    #[test]
    fn keeps_empty_messages() {
        assert_eq!(parse_log("ccc\x00\x1e\n"), vec![Commit::new("ccc", "")]);
    }

    #[test]
    fn handles_crlf_between_records() {
        let output = "aaa\x00fix: a\r\n\x1e\r\nbbb\x00fix: b\x1e";
        let commits = parse_log(output);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[1].hash, "bbb");
        assert_eq!(commits[0].message, "fix: a");
    }

    #[test]
    fn command_line_for_errors() {
        assert_eq!(
            command_line(&["tag", "--merged", "HEAD"]),
            "git tag --merged HEAD"
        );
    }
}
