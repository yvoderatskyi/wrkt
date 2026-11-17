mod config;
mod git;
mod repo;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

use config::Config;
use repo::{ClonePath, RepoName};

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
    let Err(error) = run(Cli::parse()) else {
        return ExitCode::SUCCESS;
    };
    eprintln!("error: {error:#}");
    ExitCode::FAILURE
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Repo(command) => run_repo(command),
    }
}

fn run_repo(command: RepoCommand) -> Result<()> {
    let mut config = Config::load()?;
    match command {
        RepoCommand::Add { path, name } => add_repo(&mut config, &path, name),
        RepoCommand::List => list_repos(&config),
        RepoCommand::Rm { name } => remove_repo(&mut config, &name),
    }
}

fn add_repo(config: &mut Config, path: &Path, name: Option<RepoName>) -> Result<()> {
    let clone = ClonePath::at(path)?;
    let name = name.map_or_else(|| clone.default_name(), Ok)?;
    let confirmation = format!("registered {name} -> {clone}");
    config.add_repo(name, clone)?;
    config.save()?;
    println!("{confirmation}");
    Ok(())
}

fn list_repos(config: &Config) -> Result<()> {
    print!("{}", config.describe_repos());
    Ok(())
}

fn remove_repo(config: &mut Config, name: &RepoName) -> Result<()> {
    config.remove_repo(name)?;
    config.save()?;
    println!("unregistered {name}");
    Ok(())
}
