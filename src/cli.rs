use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Calculate the next SemVer version from git commit messages.
///
/// Prints the version to stdout. Warnings and errors go to stderr. The
/// repository is only read: versinc never creates tags or commits.
#[derive(Debug, Parser)]
#[command(name = "versinc", version, args_conflicts_with_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub calculate: CalculateArgs,
}

#[derive(Debug, Args, Default)]
pub struct CalculateArgs {
    /// Config file merged on top of the global and project config
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Print versions with the configured tag prefix (e.g. v1.2.3)
    #[arg(long)]
    pub with_prefix: bool,

    /// Also write the version to this file
    #[arg(long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Format {
    /// Only the version
    #[default]
    Text,
    /// Details about the calculation as JSON
    Json,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Write a commented default config file
    ///
    /// Writes versinc.toml at the root of the current git repository, or the
    /// global config with --global.
    Init(InitArgs),

    /// Update versinc to the latest release
    ///
    /// Downloads the release archive for this platform, verifies its sha256
    /// checksum, and replaces this executable. Needs curl and tar.
    Update(UpdateArgs),
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Write the global config (~/.config/version-included/config.toml)
    #[arg(long)]
    pub global: bool,

    /// Overwrite an existing file
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Only report whether a newer release is available
    #[arg(long)]
    pub check: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("versinc").chain(args.iter().copied()))
    }

    #[test]
    fn definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn defaults() {
        let cli = parse(&[]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.calculate.config, None);
        assert!(!cli.calculate.with_prefix);
        assert_eq!(cli.calculate.output, None);
        assert_eq!(cli.calculate.format, Format::Text);
    }

    #[test]
    fn calculate_flags() {
        let cli = parse(&[
            "--config",
            "c.toml",
            "--with-prefix",
            "--output",
            "out.txt",
            "--format",
            "json",
        ])
        .unwrap();
        assert_eq!(cli.calculate.config, Some(PathBuf::from("c.toml")));
        assert!(cli.calculate.with_prefix);
        assert_eq!(cli.calculate.output, Some(PathBuf::from("out.txt")));
        assert_eq!(cli.calculate.format, Format::Json);
    }

    #[test]
    fn init_flags() {
        let Some(Command::Init(args)) = parse(&["init", "--global", "--force"]).unwrap().command
        else {
            panic!("expected init");
        };
        assert!(args.global);
        assert!(args.force);
    }

    #[test]
    fn update_flags() {
        let Some(Command::Update(args)) = parse(&["update"]).unwrap().command else {
            panic!("expected update");
        };
        assert!(!args.check);
        let Some(Command::Update(args)) = parse(&["update", "--check"]).unwrap().command else {
            panic!("expected update");
        };
        assert!(args.check);
        assert!(parse(&["update", "--force"]).is_err());
    }

    #[test]
    fn calculate_flags_are_rejected_with_init() {
        assert!(parse(&["--format", "json", "init"]).is_err());
        assert!(parse(&["init", "--format", "json"]).is_err());
    }

    #[test]
    fn rejects_unknown_format() {
        assert!(parse(&["--format", "yaml"]).is_err());
    }
}
