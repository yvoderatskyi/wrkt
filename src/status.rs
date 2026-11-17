use std::fmt;
use std::path::Path;

use anyhow::Result;

use crate::branch::Branch;
use crate::git::{self, Divergence};

pub struct RepoStatus {
    position: Position,
    state: WorktreeState,
}

impl RepoStatus {
    pub fn read(worktree: &Path) -> Result<Self> {
        let position = Position::read(worktree)?;
        let state = WorktreeState::read(worktree)?;
        Ok(RepoStatus { position, state })
    }
}

impl fmt::Display for RepoStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}  {}", self.position, self.state)
    }
}

struct WorktreeState {
    cleanliness: Cleanliness,
    rebase: Rebase,
}

impl WorktreeState {
    fn read(worktree: &Path) -> Result<Self> {
        let cleanliness = Cleanliness::read(worktree)?;
        let rebase = Rebase::read(worktree)?;
        Ok(WorktreeState {
            cleanliness,
            rebase,
        })
    }
}

impl fmt::Display for WorktreeState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}{}", self.cleanliness, self.rebase)
    }
}

enum Rebase {
    InProgress,
    None,
}

impl Rebase {
    fn read(worktree: &Path) -> Result<Self> {
        if git::is_rebasing(worktree)? {
            return Ok(Rebase::InProgress);
        }
        Ok(Rebase::None)
    }
}

impl fmt::Display for Rebase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rebase::InProgress => formatter.write_str("  rebasing"),
            Rebase::None => Ok(()),
        }
    }
}

struct Position {
    branch: Option<Branch>,
    divergences: Divergences,
}

impl Position {
    fn read(worktree: &Path) -> Result<Self> {
        let branch = Branch::checked_out_in(worktree)?;
        let divergences = Divergences::read(worktree, branch.as_ref())?;
        Ok(Position {
            branch,
            divergences,
        })
    }
}

impl fmt::Display for Position {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let branch = self
            .branch
            .as_ref()
            .map_or_else(|| "(no branch)".to_string(), Branch::to_string);
        write!(formatter, "{branch}  {}", self.divergences)
    }
}

struct Divergences {
    base: Divergence,
    remote: RemoteDivergence,
}

impl Divergences {
    fn read(worktree: &Path, branch: Option<&Branch>) -> Result<Self> {
        let tip = branch.map(Branch::as_ref);
        let base = Divergence::of_branch_from_remote(worktree, tip, git::BASE_BRANCH)?;
        let remote = RemoteDivergence::read(worktree, branch)?;
        Ok(Divergences { base, remote })
    }
}

impl fmt::Display for Divergences {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}/{} {}  {}",
            git::REMOTE,
            git::BASE_BRANCH,
            self.base,
            self.remote
        )
    }
}

enum RemoteDivergence {
    NotPushed,
    Pushed(Branch, Divergence),
}

impl RemoteDivergence {
    fn read(worktree: &Path, branch: Option<&Branch>) -> Result<Self> {
        let Some(branch) = branch else {
            return Ok(RemoteDivergence::NotPushed);
        };
        if !git::has_remote_branch(worktree, branch.as_ref())? {
            return Ok(RemoteDivergence::NotPushed);
        }
        let tip = Some(branch.as_ref());
        let divergence = Divergence::of_branch_from_remote(worktree, tip, branch.as_ref())?;
        Ok(RemoteDivergence::Pushed(branch.clone(), divergence))
    }
}

impl fmt::Display for RemoteDivergence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteDivergence::NotPushed => formatter.write_str("not pushed"),
            RemoteDivergence::Pushed(branch, divergence) => {
                write!(formatter, "{}/{branch} {divergence}", git::REMOTE)
            }
        }
    }
}

enum Cleanliness {
    Clean,
    Dirty,
}

impl Cleanliness {
    fn read(worktree: &Path) -> Result<Self> {
        if git::has_uncommitted_changes(worktree)? {
            return Ok(Cleanliness::Dirty);
        }
        Ok(Cleanliness::Clean)
    }
}

impl fmt::Display for Cleanliness {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cleanliness::Clean => formatter.write_str("clean"),
            Cleanliness::Dirty => formatter.write_str("dirty"),
        }
    }
}
