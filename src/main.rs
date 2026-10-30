use std::io;
use std::process::ExitCode;

use clap::Parser;
use version_included::app::{self, Deps};
use version_included::cli::Cli;
use version_included::env::{SystemEnv, SystemFs};
use version_included::git::SystemGit;
use version_included::update::SystemTools;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let deps = Deps {
        git: &SystemGit,
        env: &SystemEnv,
        fs: &SystemFs,
        tools: &SystemTools,
    };
    match app::run(
        &cli,
        &deps,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
