mod config;
mod git;
mod repo;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use config::Config;
use git::CloneCheck;
use repo::RepoName;

#[derive(Parser)]
#[command(
    name = "wrkt",
    about = "Manage git worktrees across several Repos as one Workspace"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the Repo registry
    #[command(subcommand)]
    Repo(RepoCommand),
}

#[derive(Subcommand)]
enum RepoCommand {
    /// Register a local clone as a Repo
    Add {
        /// Path to the clone
        path: PathBuf,
        /// Short name (defaults to the clone's directory name)
        #[arg(long)]
        name: Option<RepoName>,
    },
    /// List registered Repos
    List,
    /// Unregister a Repo
    Rm {
        /// Repo name
        name: RepoName,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Repo(cmd) => repo(cmd),
    }
}

fn repo(cmd: RepoCommand) -> Result<()> {
    let mut config = Config::load()?;
    match cmd {
        RepoCommand::Add { path, name } => {
            let clone = match git::check_clone(&path)? {
                CloneCheck::Clone(clone) => clone,
                CloneCheck::NotARepo => bail!("{} is not a git repository", path.display()),
                CloneCheck::Subdirectory { toplevel } => bail!(
                    "{} is not the top level of a clone; did you mean {}?",
                    path.display(),
                    toplevel.display()
                ),
                CloneCheck::LinkedWorktree => bail!(
                    "{} is a linked worktree; register the main clone instead",
                    path.display()
                ),
            };
            let name = match name {
                Some(name) => name,
                None => clone
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context("cannot derive a Repo name from the path; pass --name")?
                    .parse()?,
            };
            config.add_repo(name.clone(), &clone)?;
            config.save()?;
            println!("registered {name} -> {}", clone.display());
            Ok(())
        }
        RepoCommand::List => {
            let width = config
                .repos()
                .map(|(n, _)| n.as_str().len())
                .max()
                .unwrap_or(0);
            for (name, path) in config.repos() {
                println!("{name:width$}  {}", path.display());
            }
            Ok(())
        }
        RepoCommand::Rm { name } => {
            config.remove_repo(&name)?;
            config.save()?;
            println!("unregistered {name}");
            Ok(())
        }
    }
}
