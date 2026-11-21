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

    pub fn extend<'registry>(&self, clones: &Clones<'registry>) -> Result<Resolutions<'registry>> {
        clones.ensure_absent_from(&self.directory)?;
        clones.ensure_branch_free(&self.branch)?;
        clones.fetch()?;
        clones.add_worktrees(&self.directory, &self.branch)
    }
}

pub struct WorkspaceRoot(PathBuf);

impl WorkspaceRoot {
    pub fn at(root: PathBuf) -> Result<Self> {
        if !root.exists() {
            return Ok(WorkspaceRoot(root));
        }
        let resolved = root
            .canonicalize()
            .with_context(|| format!("resolving Workspace root {}", root.display()))?;
        Ok(WorkspaceRoot(resolved))
    }

    pub fn workspace(&self, name: &WorkspaceName, branch: Branch) -> Workspace {
        Workspace::new(self.0.join(name), branch)
    }

    pub fn holds(&self, name: &WorkspaceName) -> bool {
        self.0.join(name).is_dir()
    }

    pub fn directory_of(&self, name: &WorkspaceName) -> Result<PathBuf> {
        ensure!(
            self.holds(name),
            "no Workspace named '{name}' under {}",
            self.0.display()
        );
        Ok(self.0.join(name))
    }

    pub fn members(&self, name: &WorkspaceName, registry: &Registry) -> Result<Members> {
        Members::discover(&self.directory_of(name)?, registry)
    }

    pub fn workspaces(&self, registry: &Registry) -> Result<Workspaces> {
        Workspaces::discover(&self.0, registry)
    }

    pub fn containing(&self, directory: &Path) -> Result<WorkspaceName> {
        ensure!(
            directory != self.0,
            "{} is the Workspace root; cd into a Workspace or pass its name",
            self.0.display()
        );
        let child = directory
            .strip_prefix(&self.0)
            .ok()
            .and_then(|relative| relative.components().next())
            .with_context(|| self.not_holding(directory))?;
        child.as_os_str().to_string_lossy().parse()
    }

    fn not_holding(&self, directory: &Path) -> String {
        format!(
            "{} is not inside a Workspace under {}; pass a Workspace name",
            directory.display(),
            self.0.display()
        )
    }
}

pub struct Workspaces {
    root: PathBuf,
    members: BTreeMap<WorkspaceName, RepoNames>,
}

impl Workspaces {
    fn discover(root: &Path, registry: &Registry) -> Result<Self> {
        let root = root.to_path_buf();
        if !root.exists() {
            let members = BTreeMap::new();
            return Ok(Workspaces { root, members });
        }
        let members = subdirectories(&root)?
            .into_iter()
            .filter_map(|directory| directory.parse::<WorkspaceName>().ok())
            .map(|name| {
                Members::discover(&root.join(&name), registry).map(|found| (name, found.names()))
            })
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

    pub fn names(&self) -> RepoNames {
        self.0.keys().cloned().collect()
    }

    pub fn branch(&self) -> Result<Option<Branch>> {
        self.0
            .values()
            .map(|worktree| Branch::checked_out_in(worktree))
            .find_map(Result::transpose)
            .transpose()
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
