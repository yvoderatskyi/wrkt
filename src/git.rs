use std::ffi::OsStr;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, bail, ensure};

pub const REMOTE: &str = "origin";
pub const BASE_BRANCH: &str = "main";

static STREAMING: AtomicBool = AtomicBool::new(false);

pub fn stream_output() {
    STREAMING.store(true, Ordering::Relaxed);
}

#[derive(Debug)]
enum GitOutput {
    Succeeded(String),
    Failed(String),
}

fn run<Argument: AsRef<OsStr>>(directory: &Path, arguments: &[Argument]) -> Result<GitOutput> {
    let output = execute(directory, arguments)?;
    if !output.status.success() {
        let combined = [output.stdout.as_slice(), output.stderr.as_slice()].concat();
        return Ok(GitOutput::Failed(lossy(&combined)));
    }
    Ok(GitOutput::Succeeded(lossy(&output.stdout)))
}

fn execute<Argument: AsRef<OsStr>>(directory: &Path, arguments: &[Argument]) -> Result<Output> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .output()
        .with_context(|| format!("running git {}", describe(arguments)))?;
    echo(directory, arguments, &output);
    Ok(output)
}

fn echo<Argument: AsRef<OsStr>>(directory: &Path, arguments: &[Argument], output: &Output) {
    if !STREAMING.load(Ordering::Relaxed) {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    let command = format!("$ git {}  ({})\n", describe(arguments), directory.display());
    let _ = stderr.write_all(command.as_bytes());
    let _ = stderr.write_all(&output.stdout);
    let _ = stderr.write_all(&output.stderr);
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
    if !branch.is_empty() {
        return Ok(Some(branch.to_string()));
    }
    branch_being_rebased(worktree)
}

fn branch_being_rebased(worktree: &Path) -> Result<Option<String>> {
    let Some(state) = rebase_state_directory(worktree)? else {
        return Ok(None);
    };
    let head_name = state.join("head-name");
    let reference = std::fs::read_to_string(&head_name)
        .with_context(|| format!("reading {}", head_name.display()))?;
    let branch = reference.trim().strip_prefix("refs/heads/");
    Ok(branch.map(str::to_string))
}

pub fn is_rebasing(worktree: &Path) -> Result<bool> {
    Ok(rebase_state_directory(worktree)?.is_some())
}

fn rebase_state_directory(worktree: &Path) -> Result<Option<PathBuf>> {
    let stdout = run_reading(
        worktree,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "rebase-merge",
            "--git-path",
            "rebase-apply",
        ],
    )?;
    let directory = stdout.lines().map(PathBuf::from).find(|path| path.is_dir());
    Ok(directory)
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
    succeeds(clone, &["show-ref", "--verify", "--quiet", reference])
}

fn succeeds(directory: &Path, arguments: &[&str]) -> Result<bool> {
    let output = run(directory, arguments)?;
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

pub fn contains_base(worktree: &Path) -> Result<bool> {
    let base = remote_reference(BASE_BRANCH);
    succeeds(worktree, &["merge-base", "--is-ancestor", &base, "HEAD"])
}

pub fn is_contained_in_base(worktree: &Path) -> Result<bool> {
    let base = remote_reference(BASE_BRANCH);
    succeeds(worktree, &["merge-base", "--is-ancestor", "HEAD", &base])
}

pub fn is_contained_in_remote_branch(worktree: &Path, branch: &str) -> Result<bool> {
    let remote_branch = remote_reference(branch);
    succeeds(
        worktree,
        &["merge-base", "--is-ancestor", "HEAD", &remote_branch],
    )
}

pub enum RebaseEnd {
    Completed,
    Conflicted,
}

pub fn rebase_onto_base(worktree: &Path) -> Result<RebaseEnd> {
    let arguments = ["rebase".to_string(), remote_reference(BASE_BRANCH)];
    let GitOutput::Failed(output) = run(worktree, &arguments)? else {
        return Ok(RebaseEnd::Completed);
    };
    ensure!(
        is_rebasing(worktree)?,
        "git {} failed: {}",
        describe(&arguments),
        output.trim()
    );
    Ok(RebaseEnd::Conflicted)
}

pub enum PushEnd {
    Accepted(String),
    Rejected,
}

pub fn push(worktree: &Path, branch: &str) -> Result<PushEnd> {
    let arguments = [
        "push",
        "--force-with-lease",
        "--force-if-includes",
        "--set-upstream",
        REMOTE,
        branch,
    ];
    let output = execute(worktree, &arguments)?;
    let messages = lossy(&output.stderr);
    if output.status.success() {
        return Ok(PushEnd::Accepted(messages));
    }
    ensure!(
        messages.contains("[rejected]"),
        "git {} failed: {}",
        describe(&arguments),
        messages.trim()
    );
    Ok(PushEnd::Rejected)
}

pub fn clone_of(worktree: &Path) -> Result<PathBuf> {
    let stdout = run_reading(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let common_directory = PathBuf::from(stdout.trim());
    let clone = common_directory
        .parent()
        .with_context(|| format!("{} has no parent", common_directory.display()))?;
    Ok(clone.to_path_buf())
}

pub fn remove_worktree(clone: &Path, worktree: &Path) -> Result<()> {
    remove_worktree_with(clone, &[], worktree)
}

pub fn remove_worktree_discarding_changes(clone: &Path, worktree: &Path) -> Result<()> {
    remove_worktree_with(clone, &["--force"], worktree)
}

fn remove_worktree_with(clone: &Path, options: &[&str], worktree: &Path) -> Result<()> {
    let arguments: Vec<&OsStr> = ["worktree", "remove"]
        .iter()
        .chain(options)
        .map(OsStr::new)
        .chain([worktree.as_os_str()])
        .collect();
    run_checked(clone, &arguments)
}

pub fn delete_branch(clone: &Path, branch: &str) -> Result<()> {
    run_checked(clone, &["branch", "-D", branch])
}

fn run_checked<Argument: AsRef<OsStr>>(directory: &Path, arguments: &[Argument]) -> Result<()> {
    run_reading(directory, arguments).map(drop)
}

pub fn has_uncommitted_changes(worktree: &Path) -> Result<bool> {
    let stdout = run_reading(worktree, &["status", "--porcelain"])?;
    Ok(!stdout.trim().is_empty())
}

fn run_reading<Argument: AsRef<OsStr>>(directory: &Path, arguments: &[Argument]) -> Result<String> {
    match run(directory, arguments)? {
        GitOutput::Succeeded(stdout) => Ok(stdout),
        GitOutput::Failed(output) => bail!("git {} failed: {}", describe(arguments), output.trim()),
    }
}

pub struct Divergence {
    ahead: u32,
    behind: u32,
}

impl Divergence {
    pub fn of_branch_from_remote(
        worktree: &Path,
        branch: Option<&str>,
        remote_branch: &str,
    ) -> Result<Self> {
        let tip = branch.map_or_else(
            || "HEAD".to_string(),
            |branch| format!("refs/heads/{branch}"),
        );
        let range = format!("{tip}...{}", remote_reference(remote_branch));
        let stdout = run_reading(worktree, &["rev-list", "--left-right", "--count", &range])?;
        Divergence::parse(&stdout)
            .with_context(|| format!("git rev-list printed unexpected counts '{}'", stdout.trim()))
    }

    fn parse(counts: &str) -> Option<Self> {
        let mut counts = counts.split_whitespace().map(str::parse);
        let (Some(Ok(ahead)), Some(Ok(behind)), None) =
            (counts.next(), counts.next(), counts.next())
        else {
            return None;
        };
        Some(Divergence { ahead, behind })
    }
}

impl fmt::Display for Divergence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "+{} -{}", self.ahead, self.behind)
    }
}
