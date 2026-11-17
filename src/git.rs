use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

pub const REMOTE: &str = "origin";
pub const BASE_BRANCH: &str = "main";

#[derive(Debug)]
enum GitOutput {
    Succeeded(String),
    Failed(String),
}

fn run<Argument: AsRef<OsStr>>(directory: &Path, arguments: &[Argument]) -> Result<GitOutput> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .with_context(|| format!("running git {}", describe(arguments)))?;
    if !output.status.success() {
        return Ok(GitOutput::Failed(lossy(&output.stderr)));
    }
    Ok(GitOutput::Succeeded(lossy(&output.stdout)))
}

fn describe<Argument: AsRef<OsStr>>(arguments: &[Argument]) -> String {
    let words: Vec<_> = arguments
        .iter()
        .map(|argument| argument.as_ref().to_string_lossy())
        .collect();
    words.join(" ")
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

pub fn current_branch(worktree: &Path) -> Result<Option<String>> {
    let GitOutput::Succeeded(stdout) = run(worktree, &["branch", "--show-current"])? else {
        bail!("git branch --show-current failed in {}", worktree.display());
    };
    let branch = stdout.trim();
    Ok((!branch.is_empty()).then(|| branch.to_string()))
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
    let start = remote_reference(branch);
    add_worktree(clone, &["--track", "-b", branch], worktree, &start)
}

pub fn add_worktree_on_branch(clone: &Path, worktree: &Path, branch: &str) -> Result<()> {
    add_worktree(clone, &[], worktree, branch)
}

pub fn add_worktree_from_base(clone: &Path, worktree: &Path, branch: &str) -> Result<()> {
    let start = remote_reference(BASE_BRANCH);
    add_worktree(clone, &["--no-track", "-b", branch], worktree, &start)
}

fn add_worktree(clone: &Path, options: &[&str], worktree: &Path, start: &str) -> Result<()> {
    let arguments: Vec<&OsStr> = ["worktree", "add"]
        .iter()
        .chain(options)
        .map(OsStr::new)
        .chain([worktree.as_os_str(), OsStr::new(start)])
        .collect();
    run_checked(clone, &arguments)
}

fn run_checked<Argument: AsRef<OsStr>>(directory: &Path, arguments: &[Argument]) -> Result<()> {
    let GitOutput::Failed(stderr) = run(directory, arguments)? else {
        return Ok(());
    };
    bail!("git {} failed: {}", describe(arguments), stderr.trim())
}
