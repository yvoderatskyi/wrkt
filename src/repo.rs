use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Error, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::branch::Branch;
use crate::directory_name;
use crate::git;

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

    fn owns_worktree(&self, path: &Path) -> Result<bool> {
        let common_directory = git::common_directory_of_top_level(path)?;
        Ok(common_directory == Some(self.0.join(".git")))
    }

    fn fetch(&self) -> Result<()> {
        git::fetch(&self.0)
    }

    fn add_worktree(&self, worktree: &Path, branch: &Branch) -> Result<()> {
        git::add_worktree(&self.0, worktree, branch.as_ref())
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

    pub fn owner_of(&self, worktree: &Path) -> Result<Option<&RepoName>> {
        self.0
            .iter()
            .map(|(name, clone)| Ok(clone.owns_worktree(worktree)?.then_some(name)))
            .find_map(Result::transpose)
            .transpose()
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
        let width = self.0.keys().map(|name| name.0.len()).max().unwrap_or(0);
        self.0
            .iter()
            .try_for_each(|(name, clone)| writeln!(formatter, "{name:width$}  {clone}"))
    }
}

pub struct Clones<'registry>(BTreeMap<&'registry RepoName, &'registry ClonePath>);

impl Clones<'_> {
    pub fn fetch(&self) -> Result<()> {
        self.0.values().try_for_each(|clone| clone.fetch())
    }

    pub fn add_worktrees(&self, directory: &Path, branch: &Branch) -> Result<()> {
        self.0
            .iter()
            .try_for_each(|(name, clone)| clone.add_worktree(&directory.join(name), branch))
    }
}
