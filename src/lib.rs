//! A CLI tool for querying the [reproducibility] status of the Arch Linux packages
//! using data from a [rebuilderd] instance such as [reproducible.archlinux.org].
//!
//! [reproducibility]: https://reproducible-builds.org/
//! [rebuilderd]: https://wiki.archlinux.org/index.php/Rebuilderd
//! [reproducible.archlinux.org]: https://reproducible.archlinux.org/

pub mod archweb;
pub mod args;
pub mod error;
mod fetch;
pub mod package;

use alpm::{Alpm, Package as AlpmPackage, SigLevel};
use archweb::ArchwebPackage;
use args::Args;
use colored::*;
use console::{Style, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Select};
use error::ReproStatusError;
use fetch::*;
use futures::{executor, future};
use package::{LogType, Package};
use rebuilderd_common::Status;
use reqwest::Client as HttpClient;
use std::convert::TryInto;
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::str::FromStr;
use vecrem::VecExt;

/// User agent that will be used for requests.
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Presents an interactive selection dialog for providing
/// options for selecting a package and operation.
///
/// Possible operations are: showing the build logs and diffoscope.
/// It fetches the logs from rebuilderd and shows them via specified pager.
async fn inspect_packages<'a>(
    mut packages: Vec<Package>,
    default_selection: i32,
    client: &'a HttpClient,
    args: &'a Args,
) -> Result<Option<i32>, ReproStatusError> {
    if let Some(filter) = args.filter.as_ref().and_then(|s| Status::from_str(s).ok()) {
        packages.retain(|pkg| pkg.status == filter);
    }
    let items = packages
        .iter()
        .map(|pkg| pkg.to_string())
        .collect::<Vec<String>>();
    if let Some(index) = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select package to inspect")
        .default(default_selection.try_into().unwrap_or_default())
        .items(&items)
        .interact_on_opt(&Term::stderr())
        .map_or(None, |v| v)
    {
        let mut available_operations = vec!["show build log"];
        if packages[index].has_diffoscope {
            available_operations.push("show diffoscope");
        }
        available_operations.push("show package info");
        let operation = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select operation")
            .default(0)
            .items(&available_operations)
            .interact_on_opt(&Term::stderr())?;
        if operation == Some(available_operations.len() - 1) {
            println!("\n{}", packages[index].data);
            Confirm::with_theme(&ColorfulTheme {
                hint_style: Style::new().for_stderr().hidden(),
                prompt_prefix: console::style("❯".to_string()).for_stderr().green(),
                prompt_suffix: console::style(String::new()).for_stderr().hidden(),
                ..ColorfulTheme::default()
            })
            .with_prompt("Press Enter to continue")
            .wait_for_newline(true)
            .show_default(false)
            .interact_on_opt(&Term::stderr())?;
            return Ok(Some(index.try_into().unwrap_or_default()));
        }
        let log_type = match operation {
            Some(0) => LogType::Build,
            _ => LogType::Diffoscope,
        };
        let path = packages[index].get_log_path(log_type, args.cache_dir.as_ref().cloned())?;
        if path.exists() {
            log::debug!("Hit cache for {:?}", path);
        } else {
            let logs =
                fetch_rebuilderd_logs(client, &args.rebuilderd, packages[index].build_id, log_type)
                    .await?;
            fs::write(&path, logs)?;
        }
        match Command::new(&args.pager).arg(path).spawn() {
            Ok(mut child) => {
                child.wait()?;
                Ok(Some(index.try_into().unwrap_or_default()))
            }
            Err(e) => Err(ReproStatusError::IoError(e)),
        }
    } else {
        Ok(None)
    }
}

/// Prints the status of the packages to the specified output.
fn print_results<Output: Write>(
    packages: Vec<Package>,
    is_local: bool,
    output: &mut Output,
) -> Result<(), ReproStatusError> {
    let mut negatives = 0;
    for pkg in &packages {
        if pkg.status != Status::Good {
            negatives += 1;
        }
        writeln!(
            output,
            "[{}] {}",
            match pkg.status {
                Status::Good => "+".green(),
                Status::Bad => "-".red(),
                Status::Unknown => "?".yellow(),
            },
            pkg
        )?;
    }
    if packages.is_empty() {
        log::warn!("No packages found.")
    } else {
        match negatives {
            0 => log::info!("All packages are reproducible!"),
            1 => log::info!(
                "1/{} package is {} reproducible.{}",
                packages.len(),
                "not".bold(),
                String::from(if packages.len() > 1 {
                    " Almost there."
                } else {
                    ""
                }),
            ),
            _ => log::info!(
                "{}/{} packages are {} reproducible.",
                negatives,
                packages.len(),
                "not".bold(),
            ),
        }
        log::info!(
            "Your {} {:.2}% reproducible.",
            String::from(if is_local {
                "system is"
            } else {
                "packages are"
            }),
            ((packages.len() - negatives) as f64 / packages.len() as f64) * 100.
        )
    }
    Ok(())
}

/// Returns the reproducibility results of an individual maintainer's packages.
fn get_maintainer_packages<'a>(
    maintainer: &'a str,
    client: &'a HttpClient,
    args: &'a Args,
) -> Result<Vec<Package>, ReproStatusError> {
    let (archweb, rebuilderd) = executor::block_on(future::try_join(
        fetch_archweb_packages(client, maintainer),
        fetch_rebuilderd_packages(client, &args.rebuilderd),
    ))?;
    let mut packages = Vec::new();
    for pkg in archweb {
        packages.push(match rebuilderd.iter().find(|p| p.name == pkg.pkgname) {
            Some(p) => Package {
                data: pkg,
                status: p.status,
                build_id: p.build_id.unwrap_or_default(),
                has_diffoscope: p.has_diffoscope,
            },
            None => Package {
                data: pkg,
                status: Status::Unknown,
                build_id: 0,
                has_diffoscope: false,
            },
        })
    }
    Ok(packages)
}

/// Returns the reproducibility results of the locally installed packages.
fn get_user_packages<'a>(
    client: &'a HttpClient,
    args: &'a Args,
) -> Result<Vec<Package>, ReproStatusError> {
    let rebuilderd = executor::block_on(fetch_rebuilderd_packages(client, &args.rebuilderd))?;
    log::debug!("querying packages from local database: {}", args.dbpath);
    let pacman = Alpm::new("/", &args.dbpath)?;
    for repo in &args.repos {
        log::debug!("registering syncdb: {}", repo);
        pacman.register_syncdb(repo.to_string(), SigLevel::DATABASE_OPTIONAL)?;
    }
    let syncdbs = pacman.syncdbs();
    let syncpkgs = syncdbs
        .into_iter()
        .flat_map(|db| db.pkgs().iter().collect::<Vec<AlpmPackage>>())
        .collect::<Vec<AlpmPackage>>();
    let mut packages = Vec::new();
    for pkg in pacman.localdb().pkgs() {
        if !args.all && !syncpkgs.iter().any(|p| pkg.base() == p.base()) {
            continue;
        }
        packages.push(match rebuilderd.iter().find(|p| p.name == pkg.name()) {
            Some(p) => Package {
                data: ArchwebPackage::from(pkg),
                status: p.status,
                build_id: p.build_id.unwrap_or_default(),
                has_diffoscope: p.has_diffoscope,
            },
            None => Package {
                data: ArchwebPackage::from(pkg),
                status: Status::Unknown,
                build_id: 0,
                has_diffoscope: false,
            },
        });
    }
    Ok(packages)
}

/// Filters the queried packages according to the arguments specified by the user.
fn filter_packages(packages: &mut Vec<Package>, filter: Option<Status>, pkgnames: &Vec<String>) {
    let mut rem = packages.removing();
    while let Some(entry) = rem.next() {
        let package = entry.value();
        if let Some(filter) = filter {
            if package.status != filter {
                entry.remove();
                continue;
            }
        }
        if !pkgnames.is_empty() && !pkgnames.contains(&package.data.pkgname) {
            entry.remove();
        }
    }
}

/// Runs `arch-repro-status` and prints the results/shows dialogues.
pub fn run(args: Args) -> Result<(), ReproStatusError> {
    let client = HttpClient::builder().user_agent(APP_USER_AGENT).build()?;
    let mut packages = if let Some(ref maintainer) = args.maintainer {
        get_maintainer_packages(maintainer, &client, &args)
    } else {
        get_user_packages(&client, &args)
    }?;
    filter_packages(
        &mut packages,
        args.filter.as_ref().and_then(|s| Status::from_str(s).ok()),
        &args.pkgnames,
    );
    if args.inspect {
        ctrlc::set_handler(move || Term::stdout().show_cursor().expect("failed to show cursor"))?;
        let mut default_selection = Some(0);
        while let Some(selection) = default_selection {
            default_selection = executor::block_on(inspect_packages(
                packages.clone(),
                selection,
                &client,
                &args,
            ))?;
        }
        Ok(())
    } else {
        print_results(packages, args.maintainer.is_none(), &mut io::stdout())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use std::str;

    #[test]
    fn test_print_results() -> Result<()> {
        let mut output = Vec::new();
        print_results(
            vec![
                Package {
                    data: ArchwebPackage {
                        pkgname: String::from("test"),
                        pkgver: String::from("0.1"),
                        pkgrel: String::from("2"),
                        ..ArchwebPackage::default()
                    },
                    status: Status::Good,
                    build_id: 0,
                    has_diffoscope: false,
                },
                Package {
                    data: ArchwebPackage {
                        pkgname: String::from("xyz"),
                        pkgver: String::from("0.4"),
                        pkgrel: String::from("1"),
                        ..ArchwebPackage::default()
                    },
                    status: Status::Bad,
                    build_id: 0,
                    has_diffoscope: false,
                },
            ],
            false,
            &mut output,
        )?;
        assert_eq!(
            "[+] test 0.1-2 GOOD \n[-] xyz 0.4-1 BAD  \n",
            str::from_utf8(&output)?
        );
        Ok(())
    }
}
