use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Error, Result, ensure};

use crate::branch::Branch;
use crate::directory_name;
use crate::repo::{Clones, Registry, RepoName, Resolutions};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkspaceName(String);

impl FromStr for WorkspaceName {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        ensure!(
            directory_name::is_usable(name),
            "invalid Workspace name '{name}': it must be usable as a directory name"
        );
        Ok(WorkspaceName(name.to_string()))
    }
}

impl fmt::Display for WorkspaceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.pad(&self.0)
    }
}

impl AsRef<Path> for WorkspaceName {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

pub struct Workspace {
    directory: PathBuf,
    branch: Branch,
}

impl Workspace {
    pub fn new(directory: PathBuf, branch: Branch) -> Self {
        Workspace { directory, branch }
    }

    pub fn create<'registry>(&self, clones: &Clones<'registry>) -> Result<Resolutions<'registry>> {
        ensure!(
            !self.directory.exists(),
            "Workspace {} already exists; use `wrkt add` to add Repos to it",
            self.directory.display()
        );
        self.extend(clones)
    }

    pub fn existing(
        directory: PathBuf,
        registry: &Registry,
        fallback: impl FnOnce() -> Result<Branch>,
    ) -> Result<Self> {
        let branch = branch_of_members(&directory, registry)?.map_or_else(fallback, Ok)?;
        Ok(Workspace::new(directory, branch))
    }

    pub fn extend<'registry>(&self, clones: &Clones<'registry>) -> Result<Resolutions<'registry>> {
        clones.ensure_absent_from(&self.directory)?;
        clones.fetch()?;
        clones.add_worktrees(&self.directory, &self.branch)
    }
}

fn branch_of_members(directory: &Path, registry: &Registry) -> Result<Option<Branch>> {
    subdirectories(directory)?
        .into_iter()
        .map(|child| member_branch(&directory.join(child), registry))
        .find_map(Result::transpose)
        .transpose()
}

fn member_branch(worktree: &Path, registry: &Registry) -> Result<Option<Branch>> {
    if !registry.owns(worktree)? {
        return Ok(None);
    }
    Branch::checked_out_in(worktree)
}

pub struct Workspaces(BTreeMap<WorkspaceName, RepoNames>);

impl Workspaces {
    pub fn discover(root: &Path, registry: &Registry) -> Result<Self> {
        if !root.exists() {
            return Ok(Workspaces(BTreeMap::new()));
        }
        let workspaces = subdirectories(root)?
            .into_iter()
            .filter_map(|directory| directory.parse::<WorkspaceName>().ok())
            .map(|name| RepoNames::discover(&root.join(&name), registry).map(|repos| (name, repos)))
            .collect::<Result<_>>()?;
        Ok(Workspaces(workspaces))
    }
}

impl fmt::Display for Workspaces {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = self.0.keys().map(|name| name.0.len()).max().unwrap_or(0);
        self.0
            .iter()
            .try_for_each(|(name, repos)| writeln!(formatter, "{name:width$}  {repos}"))
    }
}

struct RepoNames(BTreeSet<RepoName>);

impl RepoNames {
    fn discover(workspace: &Path, registry: &Registry) -> Result<Self> {
        let names = subdirectories(workspace)?
            .into_iter()
            .map(|child| registry.owner_of(&workspace.join(child)))
            .filter_map(Result::transpose)
            .map(|owner| owner.cloned())
            .collect::<Result<_>>()?;
        Ok(RepoNames(names))
    }
}

impl fmt::Display for RepoNames {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<_> = self.0.iter().map(RepoName::to_string).collect();
        formatter.write_str(&names.join(" "))
    }
}

fn subdirectories(directory: &Path) -> Result<Vec<String>> {
    let reading = || format!("reading {}", directory.display());
    let entries = std::fs::read_dir(directory)
        .with_context(reading)?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(reading)?;
    let names = entries
        .into_iter()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    Ok(names)
}
