mod branch;
mod config;
mod directory_name;
mod git;
mod repo;
mod report;
mod status;
mod workspace;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

use branch::Branch;
use config::Config;
use repo::{ClonePath, RepoName};
use workspace::WorkspaceName;

#[derive(Parser)]
#[command(
    name = "wrkt",
    about = "Manage git worktrees across several Repos as one Workspace"
)]
struct Cli {
    /// Stream every git command and its output to stderr
    #[arg(short, long, global = true)]
    verbose: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the Repo registry
    #[command(subcommand)]
    Repo(RepoCommand),
    /// Create a Workspace with one worktree per Repo
    New {
        /// Workspace name, typically the Jira issue key
        name: WorkspaceName,
        /// Repos to include
        #[arg(required = true)]
        repos: Vec<RepoName>,
        /// Branch name (defaults to the configured branch template)
        #[arg(long)]
        branch: Option<Branch>,
    },
    /// Add Repos to an existing Workspace
    Add {
        /// Workspace name (defaults to the Workspace containing the current directory), then the Repos to add
        #[arg(required = true, value_name = "[NAME] REPOS")]
        arguments: Vec<String>,
    },
    /// List every Workspace with its Repos
    List,
    /// Show where each Repo in a Workspace stands, without fetching
    Status {
        /// Workspace name (defaults to the Workspace containing the current directory)
        name: Option<WorkspaceName>,
    },
    /// Print the Workspace a command would act on
    #[command(hide = true)]
    Which {
        /// Workspace name (defaults to the Workspace containing the current directory)
        name: Option<WorkspaceName>,
    },
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
    if cli.verbose {
        git::stream_output();
    }
    match cli.command {
        Command::Repo(command) => run_repo(command),
        Command::New {
            name,
            repos,
            branch,
        } => new_workspace(&name, &repos, branch),
        Command::Add { arguments } => extend_workspace(&arguments),
        Command::List => list_workspaces(),
        Command::Status { name } => show_status(name),
        Command::Which { name } => print_workspace(name),
    }
}

fn print_workspace(name: Option<WorkspaceName>) -> Result<()> {
    println!("{}", Config::load()?.resolve_workspace(name)?);
    Ok(())
}

fn list_workspaces() -> Result<()> {
    print!("{}", Config::load()?.workspaces()?);
    Ok(())
}

fn show_status(name: Option<WorkspaceName>) -> Result<()> {
    let report = Config::load()?.status(name)?;
    print!("{report}");
    report.ensure_succeeded()
}

fn new_workspace(name: &WorkspaceName, repos: &[RepoName], branch: Option<Branch>) -> Result<()> {
    print!("{}", Config::load()?.create_workspace(name, repos, branch)?);
    Ok(())
}

fn extend_workspace(arguments: &[String]) -> Result<()> {
    print!("{}", Config::load()?.extend_workspace(arguments)?);
    Ok(())
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
