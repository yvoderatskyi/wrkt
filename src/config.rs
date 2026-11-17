use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::branch::Branch;
use crate::push::PushOutcome;
use crate::removal::{Removal, Safety};
use crate::repo::{ClonePath, Registry, RepoName, Resolutions};
use crate::report::Report;
use crate::status::RepoStatus;
use crate::sync::SyncOutcome;
use crate::workspace::{Members, Workspace, WorkspaceName, Workspaces};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    workspace: WorkspaceSettings,
    repos: Registry,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WorkspaceSettings {
    #[serde(skip_serializing_if = "WorkspaceRoot::is_default")]
    root: WorkspaceRoot,
    branch_template: BranchTemplate,
}

impl WorkspaceSettings {
    fn workspace(&self, name: &WorkspaceName, branch: Option<Branch>) -> Result<Workspace> {
        let directory = self.root.resolved()?.join(name);
        let branch = branch.map_or_else(|| self.branch_template.branch_for(name), Ok)?;
        Ok(Workspace::new(directory, branch))
    }

    fn existing(&self, name: &WorkspaceName, registry: &Registry) -> Result<Workspace> {
        Workspace::existing(self.directory_of(name)?, registry, || {
            self.branch_template.branch_for(name)
        })
    }

    fn members(&self, name: &WorkspaceName, registry: &Registry) -> Result<Members> {
        Members::discover(&self.directory_of(name)?, registry)
    }

    fn directory_of(&self, name: &WorkspaceName) -> Result<PathBuf> {
        let root = self.root.resolved()?;
        let directory = root.join(name);
        ensure!(
            directory.is_dir(),
            "no Workspace named '{name}' under {}",
            root.display()
        );
        Ok(directory)
    }

    fn delete(&self, name: &WorkspaceName) -> Result<()> {
        let directory = self.directory_of(name)?;
        std::fs::remove_dir_all(&directory)
            .with_context(|| format!("removing Workspace {}", directory.display()))
    }

    fn containing(&self, directory: &Path) -> Result<WorkspaceName> {
        self.root.workspace_containing(directory)
    }

    fn discover(&self, registry: &Registry) -> Result<Workspaces> {
        Workspaces::discover(&self.root.resolved()?, registry)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
struct WorkspaceRoot(Option<PathBuf>);

impl WorkspaceRoot {
    fn is_default(&self) -> bool {
        self.0.is_none()
    }

    fn configured_or_default(&self) -> Result<PathBuf> {
        self.0.clone().map_or_else(|| Ok(home()?.join("wrkt")), Ok)
    }

    fn resolved(&self) -> Result<PathBuf> {
        let root = self.configured_or_default()?;
        if !root.exists() {
            return Ok(root);
        }
        root.canonicalize()
            .with_context(|| format!("resolving Workspace root {}", root.display()))
    }

    fn workspace_containing(&self, directory: &Path) -> Result<WorkspaceName> {
        let root = self.resolved()?;
        let child = directory
            .strip_prefix(&root)
            .ok()
            .and_then(|relative| relative.components().next())
            .with_context(|| not_inside_workspace(directory, &root))?;
        child.as_os_str().to_string_lossy().parse()
    }
}

fn not_inside_workspace(directory: &Path, root: &Path) -> String {
    format!(
        "{} is not inside a Workspace under {}; pass a Workspace name",
        directory.display(),
        root.display()
    )
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct BranchTemplate(String);

impl BranchTemplate {
    fn branch_for(&self, name: &WorkspaceName) -> Result<Branch> {
        self.0.replace("{key}", name.as_ref()).try_into()
    }
}

impl Default for BranchTemplate {
    fn default() -> Self {
        BranchTemplate("{key}".to_string())
    }
}

fn parse_repo_names(arguments: &[String]) -> Result<Vec<RepoName>> {
    arguments.iter().map(|argument| argument.parse()).collect()
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
        self.workspaces()?.ensure_unused_by(name)?;
        self.repos.remove(name)
    }

    pub fn create_workspace(
        &self,
        name: &WorkspaceName,
        repos: &[RepoName],
        branch: Option<Branch>,
    ) -> Result<Resolutions<'_>> {
        let workspace = self.workspace.workspace(name, branch)?;
        let clones = self.repos.select(repos)?;
        workspace.create(&clones)
    }

    pub fn extend_workspace(&self, arguments: &[String]) -> Result<Resolutions<'_>> {
        let (name, repos) = self.split_workspace_name(arguments)?;
        let name = self.resolve_workspace(name)?;
        let workspace = self.workspace.existing(&name, &self.repos)?;
        workspace.extend(&self.repos.select(&repos)?)
    }

    fn split_workspace_name(
        &self,
        arguments: &[String],
    ) -> Result<(Option<WorkspaceName>, Vec<RepoName>)> {
        let (first, rest) = arguments.split_first().context("no Repo given")?;
        if rest.is_empty() || self.repos.is_registered(first) {
            return Ok((None, parse_repo_names(arguments)?));
        }
        Ok((Some(first.parse()?), parse_repo_names(rest)?))
    }

    pub fn resolve_workspace(&self, name: Option<WorkspaceName>) -> Result<WorkspaceName> {
        name.map_or_else(|| self.current_workspace(), Ok)
    }

    fn current_workspace(&self) -> Result<WorkspaceName> {
        let directory = std::env::current_dir().context("reading the current directory")?;
        self.workspace.containing(&directory)
    }

    pub fn workspaces(&self) -> Result<Workspaces> {
        self.workspace.discover(&self.repos)
    }

    pub fn status(&self, name: Option<WorkspaceName>) -> Result<Report<RepoStatus>> {
        let name = self.resolve_workspace(name)?;
        let members = self.workspace.members(&name, &self.repos)?;
        Ok(members.status())
    }

    pub fn sync(&self, name: Option<WorkspaceName>) -> Result<Report<SyncOutcome>> {
        let name = self.resolve_workspace(name)?;
        let members = self.workspace.members(&name, &self.repos)?;
        Ok(members.sync())
    }

    pub fn push(&self, name: Option<WorkspaceName>) -> Result<Report<PushOutcome>> {
        let name = self.resolve_workspace(name)?;
        let members = self.workspace.members(&name, &self.repos)?;
        Ok(members.push())
    }

    pub fn remove(&self, name: Option<WorkspaceName>, safety: Safety) -> Result<Report<Removal>> {
        let name = self.resolve_workspace(name)?;
        let members = self.workspace.members(&name, &self.repos)?;
        let directory = self.workspace.directory_of(&name)?;
        if let Some(refusal) = members.refuse_removal(&directory, safety)? {
            return Ok(refusal);
        }
        let report = members.remove(safety);
        if report.succeeded() {
            self.workspace.delete(&name)?;
        }
        Ok(report)
    }

    pub fn describe_repos(&self) -> String {
        self.repos.to_string()
    }
}
