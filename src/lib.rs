//! Calculates the next SemVer version from git commit messages.
//!
//! The pure logic (commit parsing, classification, version calculation,
//! config merging, branch matching) lives in its own modules and never
//! touches git or the filesystem; [`git::Git`], [`env::Env`] and [`env::Fs`]
//! are the boundaries to the outside world.

pub mod app;
pub mod branch;
pub mod classify;
pub mod cli;
pub mod commit;
pub mod config;
pub mod env;
pub mod error;
pub mod git;
pub mod output;
pub mod tags;
pub mod update;
pub mod version;
