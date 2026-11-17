use std::fmt;
use std::path::Path;

use anyhow::Result;

use crate::branch::Branch;
use crate::git;
use crate::report::Success;

#[derive(Clone, Copy)]
pub enum Safety {
    Guarded,
    Forced,
}

impl Safety {
    pub fn forced_when(force: bool) -> Self {
        if force {
            return Safety::Forced;
        }
        Safety::Guarded
    }

    pub fn is_forced(self) -> bool {
        matches!(self, Safety::Forced)
    }

    fn remove_worktree(self, clone: &Path, worktree: &Path) -> Result<()> {
        match self {
            Safety::Guarded => git::remove_worktree(clone, worktree),
            Safety::Forced => git::remove_worktree_discarding_changes(clone, worktree),
        }
    }
}

pub enum Removal {
    Removable,
    Blocked(Risk),
    Removed(Option<Branch>),
}

impl Removal {
    pub fn check(worktree: &Path) -> Result<Self> {
        let Some(risk) = Risk::find(worktree)? else {
            return Ok(Removal::Removable);
        };
        Ok(Removal::Blocked(risk))
    }

    pub fn run_in(worktree: &Path, safety: Safety) -> Result<Self> {
        let clone = git::clone_of(worktree)?;
        let branch = Branch::checked_out_in(worktree)?;
        safety.remove_worktree(&clone, worktree)?;
        branch
            .as_ref()
            .map_or(Ok(()), |branch| git::delete_branch(&clone, branch))?;
        Ok(Removal::Removed(branch))
    }
}

impl Success for Removal {
    fn succeeded(&self) -> bool {
        !matches!(self, Removal::Blocked(_))
    }
}

impl fmt::Display for Removal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Removal::Removable => formatter.write_str("removable"),
            Removal::Blocked(risk) => write!(formatter, "blocked: {risk}"),
            Removal::Removed(None) => formatter.write_str("removed"),
            Removal::Removed(Some(branch)) => write!(formatter, "removed, deleted branch {branch}"),
        }
    }
}

pub enum Risk {
    UncommittedChanges,
    UnpushedCommits,
}

impl Risk {
    fn find(worktree: &Path) -> Result<Option<Self>> {
        if git::has_uncommitted_changes(worktree)? {
            return Ok(Some(Risk::UncommittedChanges));
        }
        if !is_published(worktree)? {
            return Ok(Some(Risk::UnpushedCommits));
        }
        Ok(None)
    }
}

fn is_published(worktree: &Path) -> Result<bool> {
    if git::is_contained_in_base(worktree)? {
        return Ok(true);
    }
    let Some(branch) = Branch::checked_out_in(worktree)? else {
        return Ok(false);
    };
    git::is_contained_in_remote_branch(worktree, branch.as_ref())
}

impl fmt::Display for Risk {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Risk::UncommittedChanges => formatter.write_str("uncommitted changes"),
            Risk::UnpushedCommits => formatter.write_str("unpushed commits"),
        }
    }
}
