use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::thread::{Scope, ScopedJoinHandle};

use anyhow::{Context, Error, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::branch::{Branch, Resolution};
use crate::directory_name;
use crate::git;
use crate::name_column::NameColumn;
use crate::worker::joined;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoName(String);

impl FromStr for RepoName {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        name.to_string().try_into()
    }
}

impl TryFrom<String> for RepoName {
    type Error = Error;

    fn try_from(name: String) -> Result<Self> {
        ensure!(
            directory_name::is_usable(&name),
            "invalid Repo name '{name}': it must be usable as a directory name"
        );
        Ok(RepoName(name))
    }
}

impl AsRef<Path> for RepoName {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl AsRef<str> for RepoName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<RepoName> for String {
    fn from(name: RepoName) -> Self {
        name.0
    }
}

impl fmt::Display for RepoName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.pad(&self.0)
    }
}

#[derive(Debug, Default)]
pub struct RepoNames(BTreeSet<RepoName>);

impl RepoNames {
    pub fn contains(&self, repo: &RepoName) -> bool {
        self.0.contains(repo)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn without(self, members: &RepoNames) -> RepoNames {
        self.0
            .into_iter()
            .filter(|name| !members.contains(name))
            .collect()
    }
}

impl FromIterator<RepoName> for RepoNames {
    fn from_iter<Names: IntoIterator<Item = RepoName>>(names: Names) -> Self {
        RepoNames(names.into_iter().collect())
    }
}

impl IntoIterator for RepoNames {
    type Item = RepoName;
    type IntoIter = std::collections::btree_set::IntoIter<RepoName>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl fmt::Display for RepoNames {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<_> = self.0.iter().map(RepoName::to_string).collect();
        formatter.write_str(&names.join(" "))
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClonePath(PathBuf);

impl ClonePath {
    pub fn at(path: &Path) -> Result<Self> {
        git::clone_root(path).map(ClonePath)
    }

    pub fn default_name(&self) -> Result<RepoName> {
        let directory = self
            .0
            .file_name()
            .and_then(OsStr::to_str)
            .context("cannot derive a Repo name from the path; pass --name")?;
        directory.parse()
    }

    fn shares_common_directory(&self, common_directory: &Path) -> bool {
        self.0.join(".git") == common_directory
    }

    fn fetch(&self) -> Result<()> {
        git::fetch(&self.0)
    }

    fn ensure_branch_free(&self, branch: &Branch) -> Result<()> {
        let Some(worktree) = git::worktree_holding(&self.0, branch.as_ref())? else {
            return Ok(());
        };
        bail!(
            "branch {branch} is already checked out at {}",
            worktree.display()
        )
    }

    fn add_worktree(&self, worktree: &Path, branch: &Branch) -> Result<Resolution> {
        let resolution = self.resolve(branch)?;
        resolution.add_worktree(&self.0, worktree)?;
        Ok(resolution)
    }

    fn resolve(&self, branch: &Branch) -> Result<Resolution> {
        if git::has_local_branch(&self.0, branch.as_ref())? {
            return Ok(Resolution::ReusedLocal(branch.clone()));
        }
        if git::has_remote_branch(&self.0, branch.as_ref())? {
            return Ok(Resolution::TrackingRemote(branch.clone()));
        }
        Ok(Resolution::CreatedFromBase(branch.clone()))
    }
}

impl fmt::Display for ClonePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(formatter)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Registry(BTreeMap<RepoName, ClonePath>);

impl Registry {
    pub fn add(&mut self, name: RepoName, clone: ClonePath) -> Result<()> {
        if let Some(existing) = self.0.get(&name) {
            bail!("a Repo named '{name}' is already registered ({existing})");
        }
        if let Some((other, _)) = self.0.iter().find(|(_, registered)| **registered == clone) {
            bail!("{clone} is already registered as '{other}'");
        }
        self.0.insert(name, clone);
        Ok(())
    }

    pub fn select(&self, names: &[RepoName]) -> Result<Clones<'_>> {
        let mut selected = BTreeMap::new();
        for name in names {
            let (registered, clone) = self
                .0
                .get_key_value(name)
                .with_context(|| format!("no Repo named '{name}' is registered"))?;
            ensure!(
                selected.insert(registered, clone).is_none(),
                "Repo '{name}' is named more than once"
            );
        }
        Ok(Clones(selected))
    }

    pub fn names(&self) -> RepoNames {
        self.0.keys().cloned().collect()
    }

    pub fn is_registered(&self, name: &str) -> bool {
        name.parse::<RepoName>()
            .is_ok_and(|name| self.0.contains_key(&name))
    }

    pub fn owns(&self, worktree: &Path) -> Result<bool> {
        Ok(self.owner_of(worktree)?.is_some())
    }

    pub fn owner_of(&self, worktree: &Path) -> Result<Option<&RepoName>> {
        let common_directory = git::common_directory_of_top_level(worktree)?;
        Ok(common_directory.and_then(|common_directory| self.sharing(&common_directory)))
    }

    fn sharing(&self, common_directory: &Path) -> Option<&RepoName> {
        self.0
            .iter()
            .find(|(_, clone)| clone.shares_common_directory(common_directory))
            .map(|(name, _)| name)
    }

    pub fn remove(&mut self, name: &RepoName) -> Result<()> {
        ensure!(
            self.0.remove(name).is_some(),
            "no Repo named '{name}' is registered"
        );
        Ok(())
    }
}

impl fmt::Display for Registry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let column = NameColumn::fitting(self.0.keys());
        self.0
            .iter()
            .try_for_each(|(name, clone)| column.write_row(formatter, name, clone))
    }
}

pub struct Clones<'registry>(BTreeMap<&'registry RepoName, &'registry ClonePath>);

impl<'registry> Clones<'registry> {
    pub fn fetch(&self) -> Result<()> {
        std::thread::scope(|scope| {
            self.spawn_fetches(scope)
                .into_iter()
                .try_for_each(|(name, fetch)| {
                    joined(fetch).with_context(|| format!("fetching Repo '{name}'"))
                })
        })
    }

    fn spawn_fetches<'scope>(
        &'scope self,
        scope: &'scope Scope<'scope, '_>,
    ) -> Vec<(&'registry RepoName, ScopedJoinHandle<'scope, Result<()>>)> {
        self.0
            .iter()
            .map(|(name, clone)| (*name, scope.spawn(move || clone.fetch())))
            .collect()
    }

    pub fn ensure_absent_from(&self, directory: &Path) -> Result<()> {
        let present = self.0.keys().find(|name| directory.join(name).exists());
        let Some(name) = present else {
            return Ok(());
        };
        bail!(
            "Repo '{name}' is already in Workspace {}",
            directory.display()
        )
    }

    pub fn ensure_branch_free(&self, branch: &Branch) -> Result<()> {
        self.0.iter().try_for_each(|(name, clone)| {
            clone
                .ensure_branch_free(branch)
                .with_context(|| format!("adding a worktree for Repo '{name}'"))
        })
    }

    pub fn add_worktrees(
        &self,
        directory: &Path,
        branch: &Branch,
    ) -> Result<Resolutions<'registry>> {
        self.0
            .iter()
            .map(|(name, clone)| {
                clone
                    .add_worktree(&directory.join(name), branch)
                    .with_context(|| format!("adding a worktree for Repo '{name}'"))
                    .map(|resolution| (*name, resolution))
            })
            .collect::<Result<_>>()
            .map(Resolutions)
    }
}

#[must_use]
#[derive(Debug)]
pub struct Resolutions<'registry>(BTreeMap<&'registry RepoName, Resolution>);

impl fmt::Display for Resolutions<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let column = NameColumn::fitting(self.0.keys());
        self.0
            .iter()
            .try_for_each(|(name, resolution)| column.write_row(formatter, name, resolution))
    }
}
