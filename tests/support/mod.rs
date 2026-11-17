//! Shared integration-test sandbox: a temp HOME (and so a temp config dir), a
//! temp Workspace root, and real Repos cloned from local bare `origin` repos.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

pub struct Sandbox {
    _tmp: TempDir,
    pub home: PathBuf,
    /// Where clones live (outside the Workspace root).
    pub src: PathBuf,
    /// Where bare `origin` repos live.
    pub remotes: PathBuf,
    pub workspace_root: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        let tmp = TempDir::new().expect("create temp dir");
        // Canonicalize so paths match what git reports (macOS /var -> /private/var).
        let base = tmp.path().canonicalize().expect("canonicalize temp dir");
        let home = base.join("home");
        let src = base.join("src");
        let remotes = base.join("remotes");
        let workspace_root = base.join("workspaces");
        for dir in [&home, &src, &remotes, &workspace_root] {
            std::fs::create_dir_all(dir).expect("create sandbox dir");
        }
        Sandbox {
            _tmp: tmp,
            home,
            src,
            remotes,
            workspace_root,
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.home.join(".config/wrkt/config.toml")
    }

    /// Run the `wrkt` binary inside the sandbox from `cwd`.
    pub fn wrkt_in(&self, cwd: &Path, args: &[&str]) -> Run {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_wrkt"));
        cmd.args(args).current_dir(cwd);
        self.sandbox_env(&mut cmd);
        Run(cmd.output().expect("run wrkt"))
    }

    /// Run the `wrkt` binary inside the sandbox from HOME.
    pub fn wrkt(&self, args: &[&str]) -> Run {
        self.wrkt_in(&self.home, args)
    }

    /// Run the `wrkt` binary from HOME with extra environment variables.
    pub fn wrkt_env(&self, env: &[(&str, &str)], args: &[&str]) -> Run {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_wrkt"));
        cmd.args(args).current_dir(&self.home);
        self.sandbox_env(&mut cmd);
        cmd.envs(env.iter().copied());
        Run(cmd.output().expect("run wrkt"))
    }

    /// Run plain `git` inside the sandbox; panics on failure.
    pub fn git(&self, dir: &Path, args: &[&str]) -> String {
        let mut cmd = Command::new("git");
        cmd.args(args).current_dir(dir);
        self.sandbox_env(&mut cmd);
        let out = cmd.output().expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} in {} failed:\n{}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Create a bare `origin` with one commit on `main`, and clone it to
    /// `<src>/<name>`. Returns the clone's path.
    pub fn repo(&self, name: &str) -> PathBuf {
        let origin = self.remotes.join(format!("{name}.git"));
        let seed = self.remotes.join(format!("{name}-seed"));
        let clone = self.src.join(name);
        self.git(
            &self.remotes,
            &["init", "--bare", "-b", "main", origin.to_str().unwrap()],
        );
        self.git(
            &self.remotes,
            &["init", "-b", "main", seed.to_str().unwrap()],
        );
        std::fs::write(seed.join("README.md"), format!("# {name}\n")).unwrap();
        self.git(&seed, &["add", "."]);
        self.git(&seed, &["commit", "-m", "initial"]);
        self.git(&seed, &["push", origin.to_str().unwrap(), "main"]);
        std::fs::remove_dir_all(&seed).unwrap();
        self.git(
            &self.src,
            &["clone", origin.to_str().unwrap(), clone.to_str().unwrap()],
        );
        clone
    }

    /// Write a config file pointing the Workspace root into the sandbox.
    pub fn write_config(&self, extra: &str) {
        let path = self.config_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let root = self.workspace_root.display();
        std::fs::write(&path, format!("workspace_root = \"{root}\"\n{extra}")).unwrap();
    }

    fn sandbox_env(&self, cmd: &mut Command) {
        cmd.env("HOME", &self.home)
            .env_remove("XDG_CONFIG_HOME")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.home.join(".gitconfig-none"))
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_TERMINAL_PROMPT", "0");
    }
}

/// The result of one `wrkt` invocation.
pub struct Run(pub Output);

impl Run {
    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.0.stdout).into_owned()
    }

    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.0.stderr).into_owned()
    }

    #[track_caller]
    pub fn success(&self) -> &Self {
        assert!(
            self.0.status.success(),
            "expected success, got {:?}\nstdout:\n{}\nstderr:\n{}",
            self.0.status,
            self.stdout(),
            self.stderr()
        );
        self
    }

    #[track_caller]
    pub fn failure(&self) -> &Self {
        assert!(
            !self.0.status.success(),
            "expected failure\nstdout:\n{}\nstderr:\n{}",
            self.stdout(),
            self.stderr()
        );
        self
    }
}
