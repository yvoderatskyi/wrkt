use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

pub const REMOTE: &str = "origin";
pub const BASE_BRANCH: &str = "origin/main";
const BASE_REFERENCE: &str = "refs/remotes/origin/main";

#[derive(Debug)]
enum GitOutput {
    Succeeded(String),
    Failed(String),
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

pub fn common_directory_of_top_level(path: &Path) -> Result<Option<PathBuf>> {
    let output = run(
        path,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ],
    )?;
    let GitOutput::Succeeded(stdout) = output else {
        return Ok(None);
    };
    let mut lines = stdout.lines().map(PathBuf::from);
    let toplevel = lines.next().context("git rev-parse printed no top level")?;
    let common_directory = lines
        .next()
        .context("git rev-parse printed no common directory")?;
    Ok((toplevel == path).then_some(common_directory))
}

pub fn is_valid_branch_name(name: &str) -> Result<bool> {
    let output = run(Path::new("."), &["check-ref-format", "--branch", name])?;
    Ok(matches!(output, GitOutput::Succeeded(_)))
}

pub fn fetch(clone: &Path) -> Result<()> {
    run_checked(clone, &["fetch", "--prune", REMOTE])
}

pub fn has_local_branch(clone: &Path, branch: &str) -> Result<bool> {
    has_reference(clone, &format!("refs/heads/{branch}"))
}

pub fn has_remote_branch(clone: &Path, branch: &str) -> Result<bool> {
    has_reference(clone, &remote_reference(branch))
}

fn has_reference(clone: &Path, reference: &str) -> Result<bool> {
    let output = run(clone, &["show-ref", "--verify", "--quiet", reference])?;
    Ok(matches!(output, GitOutput::Succeeded(_)))
}

fn remote_reference(branch: &str) -> String {
    format!("refs/remotes/{REMOTE}/{branch}")
}

pub fn add_worktree_tracking_remote(clone: &Path, worktree: &Path, branch: &str) -> Result<()> {
    let worktree_argument = worktree.to_string_lossy();
    let remote_branch = remote_reference(branch);
    run_checked(
        clone,
        &[
            "worktree",
            "add",
            "--track",
            "-b",
            branch,
            &worktree_argument,
            &remote_branch,
        ],
    )
}

pub fn add_worktree_on_branch(clone: &Path, worktree: &Path, branch: &str) -> Result<()> {
    let worktree_argument = worktree.to_string_lossy();
    run_checked(clone, &["worktree", "add", &worktree_argument, branch])
}

pub fn add_worktree_from_base(clone: &Path, worktree: &Path, branch: &str) -> Result<()> {
    let worktree_argument = worktree.to_string_lossy();
    run_checked(
        clone,
        &[
            "worktree",
            "add",
            "--no-track",
            "-b",
            branch,
            &worktree_argument,
            BASE_REFERENCE,
        ],
    )
}

fn run_checked(directory: &Path, arguments: &[&str]) -> Result<()> {
    let GitOutput::Failed(stderr) = run(directory, arguments)? else {
        return Ok(());
    };
    bail!("git {} failed: {}", arguments.join(" "), stderr.trim())
}
