use std::fmt;
use std::path::Path;

use anyhow::Result;

use crate::git::{self, RebaseEnd};
use crate::report::Success;

pub enum SyncOutcome {
    Rebased,
    UpToDate,
    Conflicted,
    Skipped(SkipReason),
}

impl SyncOutcome {
    pub fn run_in(worktree: &Path) -> Result<Self> {
        git::fetch(worktree)?;
        if let Some(reason) = SkipReason::find(worktree)? {
            return Ok(SyncOutcome::Skipped(reason));
        }
        if git::contains_base(worktree)? {
            return Ok(SyncOutcome::UpToDate);
        }
        match git::rebase_onto_base(worktree)? {
            RebaseEnd::Completed => Ok(SyncOutcome::Rebased),
            RebaseEnd::Conflicted => Ok(SyncOutcome::Conflicted),
        }
    }
}

impl Success for SyncOutcome {
    fn succeeded(&self) -> bool {
        !matches!(self, SyncOutcome::Conflicted)
    }
}

impl fmt::Display for SyncOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncOutcome::Skipped(reason) => write!(formatter, "skipped: {reason}"),
            SyncOutcome::Conflicted => formatter.write_str("conflicted, rebase in progress"),
            SyncOutcome::UpToDate => formatter.write_str("up to date"),
            SyncOutcome::Rebased => write!(
                formatter,
                "rebased onto {}/{}",
                git::REMOTE,
                git::BASE_BRANCH
            ),
        }
    }
}

pub enum SkipReason {
    RebaseInProgress,
    UncommittedChanges,
}

impl SkipReason {
    fn find(worktree: &Path) -> Result<Option<Self>> {
        if git::is_rebasing(worktree)? {
            return Ok(Some(SkipReason::RebaseInProgress));
        }
        if git::has_uncommitted_changes(worktree)? {
            return Ok(Some(SkipReason::UncommittedChanges));
        }
        Ok(None)
    }
}

impl fmt::Display for SkipReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkipReason::RebaseInProgress => formatter.write_str("rebase in progress"),
            SkipReason::UncommittedChanges => formatter.write_str("uncommitted changes"),
        }
    }
}
