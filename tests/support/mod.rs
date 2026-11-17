#![allow(dead_code)]

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

    pub fn write_config(&self, extra: &str) {
        let path = self.config_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let root = self.workspace_root();
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
