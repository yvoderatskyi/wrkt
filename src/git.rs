//! The git runner: the only place that spawns processes (ADR-0001).
//!
//! It exposes intent-level operations rather than raw argument lists.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

/// Captured result of one git invocation.
#[derive(Debug)]
pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    #[expect(
        dead_code,
        reason = "shown for failed Repos once multi-Repo commands land"
    )]
    pub stderr: String,
}

/// Run git in `dir`. A non-zero exit is reported in the output, not as `Err`.
///
/// # Errors
///
/// Fails if git can't be spawned.
fn run(dir: &Path, args: &[&str]) -> Result<GitOutput> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    Ok(GitOutput {
        success: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// What a path turned out to be, for registering it as a Repo.
#[derive(Debug)]
pub enum CloneCheck {
    /// The top level of a main working tree: registrable.
    Clone(PathBuf),
    /// Not inside any git working tree.
    NotARepo,
    /// Inside a working tree whose top level is elsewhere.
    Subdirectory { toplevel: PathBuf },
    /// The top level of a linked worktree, not of a clone.
    LinkedWorktree,
}

/// Classify `path` as a clone's top level, or say why it isn't one.
///
/// # Errors
///
/// Fails if git can't be spawned.
pub fn check_clone(path: &Path) -> Result<CloneCheck> {
    let Ok(path) = path.canonicalize() else {
        return Ok(CloneCheck::NotARepo);
    };
    if !path.is_dir() {
        return Ok(CloneCheck::NotARepo);
    }
    let out = run(
        &path,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-dir",
            "--git-common-dir",
        ],
    )?;
    if !out.success {
        return Ok(CloneCheck::NotARepo);
    }
    let mut lines = out.stdout.lines().map(PathBuf::from);
    let (Some(toplevel), Some(git_dir), Some(common_dir)) =
        (lines.next(), lines.next(), lines.next())
    else {
        return Ok(CloneCheck::NotARepo);
    };
    Ok(if toplevel != path {
        CloneCheck::Subdirectory { toplevel }
    } else if git_dir != common_dir {
        CloneCheck::LinkedWorktree
    } else {
        CloneCheck::Clone(toplevel)
    })
}
