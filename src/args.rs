//! Command-line argument parser.

use clap::{ArgAction, Parser};
use rebuilderd_common::Status;
use std::path::PathBuf;

/// Command-line arguments to parse.
#[derive(Debug, Parser)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    rename_all_env = "screaming-snake"
)]
pub struct Args {
    /// Disables logging.
    #[arg(short, long)]
    pub quiet: bool,
    /// Increases the logging verbosity.
    #[arg(short, long, action = ArgAction::Count, alias = "debug")]
    pub verbose: u8,
    /// Checks all of the packages on the system.
    #[arg(short, long)]
    pub all: bool,
    /// Sets the username of the maintainer.
    #[arg(short, long, value_name = "MAINTAINER", env)]
    pub maintainer: Option<String>,
    /// Sets the address of the rebuilderd instance.
    #[arg(
        short,
        long,
        value_name = "URL",
        default_value = "https://reproducible.archlinux.org",
        env
    )]
    pub rebuilderd: String,
    /// Sets the path to the pacman database.
    #[arg(
        short = 'b',
        long,
        value_name = "PATH",
        default_value = "/var/lib/pacman",
        env
    )]
    pub dbpath: String,
    /// Sets the repositories to query.
    #[arg(
        long,
        value_name = "REPO",
        default_value = "core,extra,community,multilib",
        use_value_delimiter = true
    )]
    pub repos: Vec<String>,
    /// Sets the specific packages to query for.
    #[arg(
        short = 'n',
        long,
        value_name = "PKGNAME",
        use_value_delimiter = true,
        env
    )]
    pub pkgnames: Vec<String>,
    /// Sets the filter for package status.
    #[arg(
        short,
        long,
        value_name = "STATUS",
        value_parser = *&["GOOD", "BAD", "UNKWN"],
        env
    )]
    pub filter: Option<Status>,
    /// Views the build log or diffoscope of the interactively selected package.
    #[arg(short, long)]
    pub inspect: bool,
    /// Sets the pager for viewing files.
    #[arg(short, long, value_name = "PAGER", default_value = "less", env)]
    pub pager: String,
    /// Sets the cache directory for log files.
    #[arg(short, long, value_name = "DIR", env)]
    pub cache_dir: Option<PathBuf>,
}

#[test]
#[cfg(test)]
fn verify_args() {
    use clap::CommandFactory;
    Args::command().debug_assert()
}
