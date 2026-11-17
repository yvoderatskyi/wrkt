#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

pub struct Sandbox {
    _tmp: TempDir,
    pub home: PathBuf,
    pub src: PathBuf,
    pub remotes: PathBuf,
    pub workspace_root: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        let tmp = TempDir::new().expect("create temp dir");
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

    pub fn wrkt(&self, args: &[&str]) -> Run {
        self.wrkt_with(&self.home, &[], args)
    }

    pub fn wrkt_in(&self, cwd: &Path, args: &[&str]) -> Run {
        self.wrkt_with(cwd, &[], args)
    }

    pub fn wrkt_with(&self, cwd: &Path, env: &[(&str, &str)], args: &[&str]) -> Run {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_wrkt"));
        cmd.args(args).current_dir(cwd);
        self.sandbox_env(&mut cmd);
        cmd.envs(env.iter().copied());
        Run(cmd.output().expect("run wrkt"))
    }

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
