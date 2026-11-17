use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::repo::RepoName;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    workspace_root: Option<PathBuf>,
    #[serde(default = "default_branch_template")]
    pub branch_template: String,
    #[serde(default)]
    repos: BTreeMap<RepoName, PathBuf>,
}

fn default_branch_template() -> String {
    "{key}".to_string()
}

pub fn config_path() -> Result<PathBuf> {
    let config_home = match std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => home()?.join(".config"),
    };
    Ok(config_home.join("wrkt").join("config.toml"))
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .context("HOME is not set")
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        let mut config: Config = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text)
                .with_context(|| format!("invalid config {}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => toml::from_str("")?,
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        if config.workspace_root.is_none() {
            config.workspace_root = Some(home()?.join("wrkt"));
        }
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = toml::to_string(self).context("serializing config")?;
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
    }

    pub fn repos(&self) -> impl Iterator<Item = (&RepoName, &Path)> {
        self.repos.iter().map(|(n, p)| (n, p.as_path()))
    }

    pub fn add_repo(&mut self, name: RepoName, clone: &Path) -> Result<()> {
        if let Some(existing) = self.repos.get(&name) {
            bail!(
                "a Repo named '{name}' is already registered ({})",
                existing.display()
            );
        }
        if let Some((other, _)) = self.repos.iter().find(|(_, p)| *p == clone) {
            bail!("{} is already registered as '{other}'", clone.display());
        }
        self.repos.insert(name, clone.to_path_buf());
        Ok(())
    }

    pub fn remove_repo(&mut self, name: &RepoName) -> Result<()> {
        ensure!(
            self.repos.remove(name).is_some(),
            "no Repo named '{name}' is registered"
        );
        Ok(())
    }
}
