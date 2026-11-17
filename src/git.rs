//! The git runner: the only place that spawns processes (ADR-0001).
//!
//! It exposes intent-level operations rather than raw argument lists.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Captured result of one git invocation.
pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    #[allow(
        dead_code,
        reason = "shown for failed Repos once multi-Repo commands land"
    )]
    pub stderr: String,
}

fn run(dir: &Path, args: &[&str]) -> std::io::Result<GitOutput> {
    let out = Command::new("git").args(args).current_dir(dir).output()?;
    Ok(GitOutput {
        success: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// The top-level directory of the working tree containing `path`, or `None`
/// if `path` isn't inside a git working tree.
pub fn toplevel(path: &Path) -> std::io::Result<Option<PathBuf>> {
    let out = run(path, &["rev-parse", "--show-toplevel"])?;
    Ok(out.success.then(|| PathBuf::from(out.stdout.trim())))
}
