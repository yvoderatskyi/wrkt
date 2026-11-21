use std::path::{Path, PathBuf};

use anyhow::{Context, Error, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::branch::Branch;
use crate::prompt::{self, Prompter};
use crate::push::PushOutcome;
use crate::removal::{Removal, Safety};
use crate::repo::{ClonePath, Registry, RepoName, Resolutions};
use crate::report::Report;
use crate::status::RepoStatus;
use crate::sync::SyncOutcome;
use crate::workspace::{Members, WorkspaceName, WorkspaceRoot, Workspaces};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    workspace: WorkspaceSettings,
    repos: Registry,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WorkspaceSettings {
    #[serde(skip_serializing_if = "ConfiguredRoot::is_default")]
    root: ConfiguredRoot,
    branch_template: BranchTemplate,
}

impl WorkspaceSettings {
    fn root(&self) -> Result<WorkspaceRoot> {
        self.root.resolved()
    }

    fn branch(&self, name: &WorkspaceName, given: Option<Branch>) -> Result<Branch> {
        given.map_or_else(|| self.branch_template.branch_for(name), Ok)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(try_from = "PathBuf")]
struct ConfiguredRoot(Option<PathBuf>);

impl TryFrom<PathBuf> for ConfiguredRoot {
    type Error = Error;

    fn try_from(root: PathBuf) -> Result<Self> {
        ensure!(
            root.is_absolute() || root.starts_with(HOME_SHORTHAND),
            "Workspace root '{}' must be an absolute path or start with ~/",
            root.display()
        );
        Ok(ConfiguredRoot(Some(root)))
    }
}

impl ConfiguredRoot {
    fn is_default(&self) -> bool {
        self.0.is_none()
    }

    fn resolved(&self) -> Result<WorkspaceRoot> {
        let Some(configured) = &self.0 else {
            return WorkspaceRoot::at(home()?.join("wrkt"));
        };
        let Ok(below_home) = configured.strip_prefix(HOME_SHORTHAND) else {
            return WorkspaceRoot::at(configured.clone());
        };
        WorkspaceRoot::at(home()?.join(below_home))
    }
}

const HOME_SHORTHAND: &str = "~";

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

fn named_or_current(name: Option<WorkspaceName>, root: &WorkspaceRoot) -> Result<WorkspaceName> {
    let Some(name) = name else {
        let directory = std::env::current_dir().context("reading the current directory")?;
        return root.containing(&directory);
    };
    Ok(name)
}

fn delete_workspace(directory: &Path) -> Result<()> {
    std::fs::remove_dir_all(directory)
        .with_context(|| format!("removing Workspace {}", directory.display()))
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

    pub fn remove_repo(
        &mut self,
        given: Option<RepoName>,
        prompter: &mut dyn Prompter,
    ) -> Result<RepoName> {
        let name = prompt::removed_repo(given, &self.repos, prompter)?;
        self.workspaces()?.ensure_unused_by(&name)?;
        self.repos.remove(&name)?;
        Ok(name)
    }

    pub fn create_workspace(
        &self,
        name: Option<WorkspaceName>,
        repos: Vec<RepoName>,
        branch: Option<Branch>,
        prompter: &mut dyn Prompter,
    ) -> Result<Resolutions<'_>> {
        let (name, repos) = prompt::new_workspace_arguments(name, repos, &self.repos, prompter)?;
        let root = self.workspace.root()?;
        let workspace = root.workspace(&name, self.workspace.branch(&name, branch)?);
        let clones = self.repos.select(&repos)?;
        workspace.create(&clones)
    }

    pub fn extend_workspace(
        &self,
        arguments: &[String],
        prompter: &mut dyn Prompter,
    ) -> Result<Resolutions<'_>> {
        let root = self.workspace.root()?;
        let (name, repos) = self.split_workspace_name(arguments, &root)?;
        let name = named_or_current(name, &root)?;
        let members = root.members(&name, &self.repos)?;
        let repos = prompt::added_repos(repos, &name, &members.names(), &self.repos, prompter)?;
        let branch = self.workspace.branch(&name, members.branch()?)?;
        let workspace = root.workspace(&name, branch);
        workspace.extend(&self.repos.select(&repos)?)
    }

    fn split_workspace_name(
        &self,
        arguments: &[String],
        root: &WorkspaceRoot,
    ) -> Result<(Option<WorkspaceName>, Vec<RepoName>)> {
        let Some((first, rest)) = arguments.split_first() else {
            return Ok((None, Vec::new()));
        };
        if rest.is_empty() && self.names_workspace(first, root) {
            return Ok((Some(first.parse()?), Vec::new()));
        }
        if rest.is_empty() || self.repos.is_registered(first) {
            return Ok((None, parse_repo_names(arguments)?));
        }
        Ok((Some(first.parse()?), parse_repo_names(rest)?))
    }

    fn names_workspace(&self, argument: &str, root: &WorkspaceRoot) -> bool {
        !self.repos.is_registered(argument) && argument.parse().is_ok_and(|name| root.holds(&name))
    }

    pub fn resolve_workspace(&self, name: Option<WorkspaceName>) -> Result<WorkspaceName> {
        let Some(name) = name else {
            return named_or_current(None, &self.workspace.root()?);
        };
        Ok(name)
    }

    pub fn workspaces(&self) -> Result<Workspaces> {
        self.workspace.root()?.workspaces(&self.repos)
    }

    pub fn status(&self, name: Option<WorkspaceName>) -> Result<Report<RepoStatus>> {
        Ok(self.members(name)?.status())
    }

    pub fn sync(&self, name: Option<WorkspaceName>) -> Result<Report<SyncOutcome>> {
        Ok(self.members(name)?.sync())
    }

    pub fn push(&self, name: Option<WorkspaceName>) -> Result<Report<PushOutcome>> {
        Ok(self.members(name)?.push())
    }

    fn members(&self, name: Option<WorkspaceName>) -> Result<Members> {
        let root = self.workspace.root()?;
        root.members(&named_or_current(name, &root)?, &self.repos)
    }

    pub fn remove(&self, name: Option<WorkspaceName>, safety: Safety) -> Result<Report<Removal>> {
        let root = self.workspace.root()?;
        let directory = root.directory_of(&named_or_current(name, &root)?)?;
        let members = Members::discover(&directory, &self.repos)?;
        if let Some(refusal) = members.refuse_removal(&directory, safety)? {
            return Ok(refusal);
        }
        let report = members.remove(safety);
        if report.succeeded() {
            delete_workspace(&directory)?;
        }
        Ok(report)
    }

    pub fn describe_repos(&self) -> String {
        self.repos.to_string()
    }
}
