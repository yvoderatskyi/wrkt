mod config;
mod git;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use config::Config;

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
        name: Option<String>,
    },
    /// List registered Repos
    List,
    /// Unregister a Repo
    Rm {
        /// Repo name
        name: String,
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
            let toplevel = git::toplevel(&path)
                .ok()
                .flatten()
                .with_context(|| format!("{} is not a git repository", path.display()))?;
            let name = match name {
                Some(name) => name,
                None => toplevel
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context("cannot derive a Repo name from the path; pass --name")?
                    .to_string(),
            };
            if let Some(existing) = config.repos.get(&name) {
                bail!(
                    "a Repo named '{name}' is already registered ({})",
                    existing.display()
                );
            }
            println!("registered {name} -> {}", toplevel.display());
            config.repos.insert(name, toplevel);
            config.save()
        }
        RepoCommand::List => {
            let width = config.repos.keys().map(String::len).max().unwrap_or(0);
            for (name, path) in &config.repos {
                println!("{name:width$}  {}", path.display());
            }
            Ok(())
        }
        RepoCommand::Rm { name } => {
            if config.repos.remove(&name).is_none() {
                bail!("no Repo named '{name}' is registered");
            }
            config.save()?;
            println!("unregistered {name}");
            Ok(())
        }
    }
}
