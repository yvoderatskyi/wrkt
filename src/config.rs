use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::branch::Branch;
use crate::repo::{ClonePath, Registry, RepoName};
use crate::workspace::{Workspace, WorkspaceName, Workspaces};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    workspace: WorkspaceSettings,
    repos: Registry,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WorkspaceSettings {
    root: WorkspaceRoot,
    branch_template: BranchTemplate,
}

impl WorkspaceSettings {
    fn workspace(&self, name: &WorkspaceName, branch: Option<Branch>) -> Result<Workspace> {
        let directory = self.root.resolved()?.join(name);
        let branch = branch.map_or_else(|| self.branch_template.branch_for(name), Ok)?;
        Ok(Workspace::new(directory, branch))
    }

    fn discover(&self, registry: &Registry) -> Result<Workspaces> {
        Workspaces::discover(&self.root.resolved()?, registry)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct WorkspaceRoot(PathBuf);

impl WorkspaceRoot {
    fn resolved(&self) -> Result<PathBuf> {
        if !self.0.exists() {
            return Ok(self.0.clone());
        }
        self.0
            .canonicalize()
            .with_context(|| format!("resolving Workspace root {}", self.0.display()))
    }
}

impl Default for WorkspaceRoot {
    fn default() -> Self {
        let home_or_working_directory = std::env::home_dir().unwrap_or_default();
        WorkspaceRoot(home_or_working_directory.join("wrkt"))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct BranchTemplate(String);

impl BranchTemplate {
    fn branch_for(&self, name: &WorkspaceName) -> Result<Branch> {
        self.0.replace("{key}", &name.to_string()).try_into()
    }
}

impl Default for BranchTemplate {
    fn default() -> Self {
        BranchTemplate("{key}".to_string())
    }
}

fn config_path() -> Result<PathBuf> {
    let config_home = match std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        Some(directory) => PathBuf::from(directory),
        None => home()?.join(".config"),
    };
    Ok(config_home.join("wrkt").join("config.toml"))
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .context("HOME is not set")
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        let read = std::fs::read_to_string(&path);
        if read
            .as_ref()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        {
            return Ok(Config::default());
        }
        let text = read.with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("invalid config {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        let directory = path
            .parent()
            .context("config path has no parent directory")?;
        std::fs::create_dir_all(directory)
            .with_context(|| format!("creating {}", directory.display()))?;
        let text = toml::to_string(self).context("serializing config")?;
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
    }

    pub fn add_repo(&mut self, name: RepoName, clone: ClonePath) -> Result<()> {
        self.repos.add(name, clone)
    }

    pub fn remove_repo(&mut self, name: &RepoName) -> Result<()> {
        self.repos.remove(name)
    }

    pub fn create_workspace(
        &self,
        name: &WorkspaceName,
        repos: &[RepoName],
        branch: Option<Branch>,
    ) -> Result<()> {
        let workspace = self.workspace.workspace(name, branch)?;
        let clones = self.repos.select(repos)?;
        workspace.create(&clones)
    }

    pub fn workspaces(&self) -> Result<Workspaces> {
        self.workspace.discover(&self.repos)
    }

    pub fn describe_repos(&self) -> String {
        self.repos.to_string()
    }
}
