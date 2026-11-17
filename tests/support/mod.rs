#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

pub struct Sandbox {
    _temporary: TempDir,
    base: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        let temporary = TempDir::new().expect("create temporary directory");
        let base = temporary
            .path()
            .canonicalize()
            .expect("resolve the temporary directory's symlinks (macOS /var -> /private/var) so paths match what git reports");
        let sandbox = Sandbox {
            _temporary: temporary,
            base,
        };
        for directory in [sandbox.home(), sandbox.clones(), sandbox.remotes()] {
            std::fs::create_dir_all(directory).expect("create sandbox directory");
        }
        sandbox
    }

    pub fn home(&self) -> PathBuf {
        self.base.join("home")
    }

    pub fn clones(&self) -> PathBuf {
        self.base.join("clones")
    }

    pub fn workspace_root(&self) -> PathBuf {
        self.base.join("workspaces")
    }

    pub fn config_path(&self) -> PathBuf {
        self.home().join(".config/wrkt/config.toml")
    }

    fn remotes(&self) -> PathBuf {
        self.base.join("remotes")
    }

    pub fn wrkt(&self, arguments: &[&str]) -> Run {
        self.wrkt_with(&self.home(), &[], arguments)
    }

    pub fn wrkt_in(&self, working_directory: &Path, arguments: &[&str]) -> Run {
        self.wrkt_with(working_directory, &[], arguments)
    }

    pub fn wrkt_with(
        &self,
        working_directory: &Path,
        environment: &[(&str, &str)],
        arguments: &[&str],
    ) -> Run {
        let mut command = Command::new(env!("CARGO_BIN_EXE_wrkt"));
        command.args(arguments).current_dir(working_directory);
        self.isolate(&mut command);
        command.envs(environment.iter().copied());
        Run(command.output().expect("run wrkt"))
    }

    pub fn git(&self, directory: &Path, arguments: &[&str]) -> String {
        let mut command = Command::new("git");
        command.args(arguments).current_dir(directory);
        self.isolate(&mut command);
        let run = Run(command.output().expect("run git"));
        run.success();
        run.stdout().trim().to_string()
    }

    pub fn local_branch(&self, clone: &Path, branch: &str) -> String {
        self.git(
            clone,
            &["branch", "--list", "--format=%(refname:short)", branch],
        )
    }

    pub fn repo(&self, name: &str) -> PathBuf {
        let remotes = self.remotes();
        let origin = remotes.join(format!("{name}.git"));
        let seed = remotes.join(format!("{name}-seed"));
        let clone = self.clones().join(name);
        self.git(
            &remotes,
            &["init", "--bare", "-b", "main", origin.to_str().unwrap()],
        );
        self.git(&remotes, &["init", "-b", "main", seed.to_str().unwrap()]);
        std::fs::write(seed.join("README.md"), format!("# {name}\n")).unwrap();
        self.git(&seed, &["add", "."]);
        self.git(&seed, &["commit", "-m", "initial"]);
        self.git(&seed, &["push", origin.to_str().unwrap(), "main"]);
        std::fs::remove_dir_all(&seed).unwrap();
        self.git(
            &self.clones(),
            &["clone", origin.to_str().unwrap(), clone.to_str().unwrap()],
        );
        clone
    }

    pub fn registered_repo(&self, name: &str) -> PathBuf {
        if !self.config_path().exists() {
            self.write_config("");
        }
        let clone = self.repo(name);
        self.wrkt(&["repo", "add", clone.to_str().unwrap()])
            .success();
        clone
    }

    pub fn advance_origin_main(&self, name: &str) -> String {
        self.push_colleague_commit(name, "main")
    }

    pub fn push_colleague_commit(&self, name: &str, branch: &str) -> String {
        self.push_colleague_commit_from(name, "main", branch)
    }

    pub fn extend_origin_branch(&self, name: &str, branch: &str) -> String {
        self.push_colleague_commit_from(name, branch, branch)
    }

    fn push_colleague_commit_from(&self, name: &str, start: &str, branch: &str) -> String {
        let remotes = self.remotes();
        let origin = remotes.join(format!("{name}.git"));
        let colleague = remotes.join(format!("{name}-colleague"));
        self.git(
            &remotes,
            &[
                "clone",
                "--branch",
                start,
                origin.to_str().unwrap(),
                colleague.to_str().unwrap(),
            ],
        );
        std::fs::write(colleague.join("CHANGE.md"), "colleague change\n").unwrap();
        self.git(&colleague, &["add", "."]);
        self.git(&colleague, &["commit", "-m", "colleague change"]);
        self.git(&colleague, &["push", "origin", &format!("HEAD:{branch}")]);
        let commit = self.git(&colleague, &["rev-parse", "HEAD"]);
        std::fs::remove_dir_all(&colleague).unwrap();
        commit
    }

    pub fn print_on_origin_push(&self, name: &str, message: &str) {
        let hook = self
            .remotes()
            .join(format!("{name}.git/hooks/post-receive"));
        std::fs::write(&hook, format!("#!/bin/sh\ncat <<'EOF'\n{message}\nEOF\n")).unwrap();
        let executable = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&hook, executable).unwrap();
    }

    pub fn delete_origin_branch(&self, name: &str, branch: &str) {
        let origin = self.remotes().join(format!("{name}.git"));
        self.git(&origin, &["branch", "-D", branch]);
    }

    pub fn commit_on_local_branch(&self, clone: &Path, branch: &str) -> String {
        self.git(clone, &["switch", "-c", branch]);
        std::fs::write(clone.join("LOCAL.md"), "local work\n").unwrap();
        self.git(clone, &["add", "."]);
        self.git(clone, &["commit", "-m", "local work"]);
        let commit = self.git(clone, &["rev-parse", "HEAD"]);
        self.git(clone, &["switch", "main"]);
        commit
    }

    pub fn commit_in(&self, worktree: &Path, file: &str) -> String {
        std::fs::write(worktree.join(file), format!("{file}\n")).unwrap();
        self.git(worktree, &["add", "."]);
        self.git(worktree, &["commit", "-m", file]);
        self.git(worktree, &["rev-parse", "HEAD"])
    }

    pub fn rebase_into_conflict(&self, name: &str, clone: &Path, worktree: &Path) {
        self.commit_in(worktree, "CHANGE.md");
        self.advance_origin_main(name);
        self.git(clone, &["fetch", "origin"]);
        let mut command = Command::new("git");
        command
            .args(["rebase", "origin/main"])
            .current_dir(worktree);
        self.isolate(&mut command);
        Run(command.output().expect("run git")).failure();
    }

    pub fn break_origin(&self, clone: &Path) {
        self.git(
            clone,
            &["remote", "set-url", "origin", "/nonexistent/origin.git"],
        );
    }

    pub fn probe_origin(&self, clone: &Path) {
        let helpers = self.base.join("helpers");
        std::fs::create_dir_all(&helpers).unwrap();
        let helper = helpers.join("git-remote-probe");
        let script = "#!/bin/sh\necho \"probed GIT_TERMINAL_PROMPT=$GIT_TERMINAL_PROMPT LC_ALL=$LC_ALL\" >&2\nexit 1\n";
        std::fs::write(&helper, script).unwrap();
        let executable = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&helper, executable).unwrap();
        self.git(clone, &["remote", "set-url", "origin", "probe::origin"]);
    }

    pub fn path_with_probe(&self) -> String {
        let helpers = self.base.join("helpers");
        let inherited = std::env::var("PATH").unwrap_or_default();
        format!("{}:{inherited}", helpers.display())
    }

    pub fn write_config(&self, extra: &str) {
        self.write_config_rooted_at(&self.workspace_root(), extra);
    }

    pub fn write_config_rooted_at(&self, root: &Path, extra: &str) {
        let path = self.config_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = format!("[workspace]\nroot = \"{}\"\n{extra}", root.display());
        std::fs::write(&path, text).unwrap();
    }

    fn isolate(&self, command: &mut Command) {
        command
            .env("HOME", self.home())
            .env_remove("XDG_CONFIG_HOME")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.home().join(".gitconfig-none"))
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_TERMINAL_PROMPT", "0");
    }
}

pub struct Run(Output);

impl Run {
    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.0.stdout).into_owned()
    }

    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.0.stderr).into_owned()
    }

    #[track_caller]
    pub fn line_for(&self, repo: &str) -> String {
        let stdout = self.stdout();
        let line = stdout
            .lines()
            .find(|line| line.split_whitespace().next() == Some(repo));
        line.unwrap_or_else(|| panic!("no line for {repo} in {self}"))
            .to_string()
    }

    pub fn succeeded(&self) -> bool {
        self.0.status.success()
    }

    #[track_caller]
    pub fn success(&self) -> &Self {
        assert!(self.succeeded(), "expected success, got {self}");
        self
    }

    #[track_caller]
    pub fn failure(&self) -> &Self {
        assert!(!self.succeeded(), "expected failure, got {self}");
        self
    }
}

impl std::fmt::Display for Run {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}\nstdout:\n{}\nstderr:\n{}",
            self.0.status,
            self.stdout(),
            self.stderr()
        )
    }
}
