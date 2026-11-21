use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use std::thread::Scope;

use anyhow::{Context, Error, Result, bail, ensure};

use crate::branch::Branch;
use crate::directory_name;
use crate::name_column::NameColumn;
use crate::push::PushOutcome;
use crate::removal::{Removal, Safety};
use crate::repo::{Clones, Registry, RepoName, RepoNames, Resolutions};
use crate::report::Report;
use crate::status::RepoStatus;
use crate::sync::SyncOutcome;
use crate::worker::joined;

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

impl AsRef<str> for WorkspaceName {
    fn as_ref(&self) -> &str {
        &self.0
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
        clones.ensure_branch_free(&self.branch)?;
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

pub struct Workspaces {
    root: PathBuf,
    members: BTreeMap<WorkspaceName, RepoNames>,
}

impl Workspaces {
    pub fn discover(root: &Path, registry: &Registry) -> Result<Self> {
        let root = root.to_path_buf();
        if !root.exists() {
            let members = BTreeMap::new();
            return Ok(Workspaces { root, members });
        }
        let members = subdirectories(&root)?
            .into_iter()
            .filter_map(|directory| directory.parse::<WorkspaceName>().ok())
            .map(|name| member_names(&root.join(&name), registry).map(|repos| (name, repos)))
            .collect::<Result<_>>()?;
        Ok(Workspaces { root, members })
    }

    pub fn ensure_unused_by(&self, repo: &RepoName) -> Result<()> {
        let using_workspace = self.members.iter().find(|(_, repos)| repos.contains(repo));
        let Some((name, _)) = using_workspace else {
            return Ok(());
        };
        bail!("Repo '{repo}' is still in Workspace {name}; run `wrkt rm {name}` first")
    }

    pub fn hint_when_empty(&self) -> Option<String> {
        let root = self.root.display();
        self.members
            .is_empty()
            .then(|| format!("No Workspaces under {root} yet; create one with wrkt new <name>"))
    }
}

impl fmt::Display for Workspaces {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let column = NameColumn::fitting(self.members.keys());
        self.members
            .iter()
            .try_for_each(|(name, repos)| column.write_row(formatter, name, repos))
    }
}

pub struct Members(BTreeMap<RepoName, PathBuf>);

impl Members {
    pub fn discover(workspace: &Path, registry: &Registry) -> Result<Self> {
        let members = subdirectories(workspace)?
            .into_iter()
            .map(|child| member(workspace.join(child), registry))
            .filter_map(Result::transpose)
            .collect::<Result<_>>()?;
        Ok(Members(members))
    }

    pub fn status(&self) -> Report<RepoStatus> {
        self.in_parallel(RepoStatus::read)
    }

    pub fn sync(&self) -> Report<SyncOutcome> {
        self.in_parallel(SyncOutcome::run_in)
    }

    pub fn push(&self) -> Report<PushOutcome> {
        self.in_parallel(PushOutcome::run_in)
    }

    pub fn refuse_removal(
        &self,
        workspace: &Path,
        safety: Safety,
    ) -> Result<Option<Report<Removal>>> {
        if safety.is_forced() {
            return Ok(None);
        }
        self.ensure_nothing_else_in(workspace)?;
        let check = self.in_parallel(Removal::check);
        Ok((!check.succeeded()).then_some(check))
    }

    fn ensure_nothing_else_in(&self, workspace: &Path) -> Result<()> {
        let strays: Vec<_> = entry_names(workspace)?
            .into_iter()
            .filter(|entry| entry != FINDER_METADATA)
            .filter(|entry| !self.holds(&workspace.join(entry)))
            .collect();
        ensure!(
            strays.is_empty(),
            "Workspace {} holds {} outside any Repo; move it out or pass --force",
            workspace.display(),
            strays.join(", ")
        );
        Ok(())
    }

    fn holds(&self, worktree: &Path) -> bool {
        self.0.values().any(|member| member == worktree)
    }

    pub fn remove(&self, safety: Safety) -> Report<Removal> {
        self.in_parallel(|worktree| Removal::run_in(worktree, safety))
    }

    fn in_parallel<Outcome: Send>(
        &self,
        work: impl Fn(&Path) -> Result<Outcome> + Sync,
    ) -> Report<Outcome> {
        std::thread::scope(|scope| self.spawn_each(scope, &work).collect())
    }

    fn spawn_each<'scope, Outcome: Send + 'scope>(
        &'scope self,
        scope: &'scope Scope<'scope, '_>,
        work: &'scope (impl Fn(&Path) -> Result<Outcome> + Sync),
    ) -> impl Iterator<Item = (RepoName, Result<Outcome>)> {
        let handles: Vec<_> = self
            .0
            .iter()
            .map(|(name, worktree)| (name, scope.spawn(move || work(worktree))))
            .collect();
        handles
            .into_iter()
            .map(|(name, handle)| (name.clone(), joined(handle)))
    }
}

fn member(worktree: PathBuf, registry: &Registry) -> Result<Option<(RepoName, PathBuf)>> {
    let owner = registry.owner_of(&worktree)?.cloned();
    Ok(owner.map(|name| (name, worktree)))
}

pub fn member_names(workspace: &Path, registry: &Registry) -> Result<RepoNames> {
    subdirectories(workspace)?
        .into_iter()
        .map(|child| registry.owner_of(&workspace.join(child)))
        .filter_map(Result::transpose)
        .map(Result::<&RepoName, Error>::cloned)
        .collect()
}

const FINDER_METADATA: &str = ".DS_Store";

fn subdirectories(directory: &Path) -> Result<Vec<String>> {
    let names = entry_names(directory)?
        .into_iter()
        .filter(|name| directory.join(name).is_dir())
        .collect();
    Ok(names)
}

fn entry_names(directory: &Path) -> Result<Vec<String>> {
    let reading = || format!("reading {}", directory.display());
    let entries = std::fs::read_dir(directory)
        .with_context(reading)?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(reading)?;
    let names = entries
        .into_iter()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    Ok(names)
}
