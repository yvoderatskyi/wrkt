use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

#[derive(Debug)]
enum GitOutput {
    Succeeded(String),
    Failed(
        #[expect(
            dead_code,
            reason = "shown for failed Repos once multi-Repo commands land"
        )]
        String,
    ),
}

fn run(directory: &Path, arguments: &[&str]) -> Result<GitOutput> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .with_context(|| format!("running git {}", arguments.join(" ")))?;
    if !output.status.success() {
        return Ok(GitOutput::Failed(lossy(&output.stderr)));
    }
    Ok(GitOutput::Succeeded(lossy(&output.stdout)))
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub fn clone_root(path: &Path) -> Result<PathBuf> {
    let not_a_repository = || format!("{} is not a git repository", path.display());
    let canonical = path.canonicalize().with_context(not_a_repository)?;
    ensure!(canonical.is_dir(), not_a_repository());
    let GitOutput::Succeeded(stdout) = run(
        &canonical,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-dir",
            "--git-common-dir",
        ],
    )?
    else {
        bail!(not_a_repository());
    };
    let mut lines = stdout.lines().map(PathBuf::from);
    let (Some(toplevel), Some(git_directory), Some(common_directory)) =
        (lines.next(), lines.next(), lines.next())
    else {
        bail!(not_a_repository());
    };
    ensure!(
        toplevel == canonical,
        "{} is not the top level of a clone; did you mean {}?",
        path.display(),
        toplevel.display()
    );
    ensure!(
        git_directory == common_directory,
        "{} is a linked worktree; register the main clone instead",
        path.display()
    );
    Ok(canonical)
}
