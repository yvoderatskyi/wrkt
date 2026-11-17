//! Repo names: validated once at the boundary (CLI argument, config file).

use std::fmt;
use std::str::FromStr;

use anyhow::{Error, ensure};
use serde::{Deserialize, Serialize};

/// A registered Repo's short name. It is always usable as a single directory
/// name, since each Repo becomes `<Workspace>/<name>/`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RepoName(String);

impl RepoName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for RepoName {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        name.to_string().try_into()
    }
}

impl TryFrom<String> for RepoName {
    type Error = Error;

    fn try_from(name: String) -> Result<Self, Self::Error> {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&self.0)
    }
}
