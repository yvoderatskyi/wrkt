use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Error, Result, bail, ensure};
use serde::{Deserialize, Serialize};

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
            !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\']),
            "invalid Repo name '{name}': it must be usable as a directory name"
        );
        Ok(RepoName(name))
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
