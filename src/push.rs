use std::fmt;
use std::path::Path;

use anyhow::{Context, Result};

use crate::branch::Branch;
use crate::git::{self, PushEnd};
use crate::report::Success;

pub enum PushOutcome {
    Pushed(Option<PullRequestUrl>),
    NothingToPush,
    SkippedMidRebase,
    Rejected(Branch),
}

impl PushOutcome {
    pub fn run_in(worktree: &Path) -> Result<Self> {
        if git::is_rebasing(worktree)? {
            return Ok(PushOutcome::SkippedMidRebase);
        }
        if git::is_contained_in_base(worktree)? {
            return Ok(PushOutcome::NothingToPush);
        }
        let branch = Branch::checked_out_in(worktree)?
            .with_context(|| format!("{} has no branch checked out", worktree.display()))?;
        match git::push(worktree, &branch)? {
            PushEnd::Accepted(messages) => {
                Ok(PushOutcome::Pushed(PullRequestUrl::find_in(&messages)))
            }
            PushEnd::Rejected => Ok(PushOutcome::Rejected(branch)),
        }
    }
}

impl Success for PushOutcome {
    fn succeeded(&self) -> bool {
        !matches!(self, PushOutcome::Rejected(_))
    }
}

impl fmt::Display for PushOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushOutcome::Pushed(None) => formatter.write_str("pushed"),
            PushOutcome::Pushed(Some(url)) => write!(formatter, "pushed  {url}"),
            PushOutcome::Rejected(branch) => write!(
                formatter,
                "rejected: {}/{branch} has commits your branch lacks; integrate them first",
                git::REMOTE
            ),
            PushOutcome::SkippedMidRebase => formatter.write_str("skipped: rebase in progress"),
            PushOutcome::NothingToPush => write!(
                formatter,
                "nothing to push beyond {}/{}",
                git::REMOTE,
                git::BASE_BRANCH
            ),
        }
    }
}

pub struct PullRequestUrl(String);

impl PullRequestUrl {
    fn find_in(messages: &str) -> Option<Self> {
        messages
            .lines()
            .filter_map(|line| line.strip_prefix("remote:"))
            .flat_map(str::split_whitespace)
            .find(|word| word.starts_with("https://") || word.starts_with("http://"))
            .map(|url| PullRequestUrl(url.to_string()))
    }
}

impl fmt::Display for PullRequestUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
