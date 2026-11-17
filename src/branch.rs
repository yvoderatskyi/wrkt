use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Error, Result, ensure};

use crate::git;

#[derive(Debug, Clone)]
pub struct Branch(String);

impl Branch {
    pub fn checked_out_in(worktree: &Path) -> Result<Option<Branch>> {
        Ok(git::current_branch(worktree)?.map(Branch))
    }
}

impl FromStr for Branch {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        name.to_string().try_into()
    }
}

impl TryFrom<String> for Branch {
    type Error = Error;

    fn try_from(name: String) -> Result<Self> {
        ensure!(
            git::is_valid_branch_name(&name)?,
            "invalid branch name '{name}'"
        );
        Ok(Branch(name))
    }
}

impl AsRef<str> for Branch {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Branch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.pad(&self.0)
    }
}

#[derive(Debug)]
pub enum Resolution {
    ReusedLocal(Branch),
    TrackingRemote(Branch),
    CreatedFromBase(Branch),
}

impl Resolution {
    pub fn add_worktree(&self, clone: &Path, worktree: &Path) -> Result<()> {
        match self {
            Resolution::ReusedLocal(branch) => {
                git::add_worktree_on_branch(clone, worktree, branch.as_ref())
            }
            Resolution::TrackingRemote(branch) => {
                git::add_worktree_tracking_remote(clone, worktree, branch.as_ref())
            }
            Resolution::CreatedFromBase(branch) => {
                git::add_worktree_from_base(clone, worktree, branch.as_ref())
            }
        }
    }
}

impl fmt::Display for Resolution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Resolution::ReusedLocal(branch) => write!(formatter, "reused local branch {branch}"),
            Resolution::TrackingRemote(branch) => {
                write!(formatter, "tracking {}/{branch}", git::REMOTE)
            }
            Resolution::CreatedFromBase(branch) => {
                write!(
                    formatter,
                    "created {branch} from {}/{}",
                    git::REMOTE,
                    git::BASE_BRANCH
                )
            }
        }
    }
}
