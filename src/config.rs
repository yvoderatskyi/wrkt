//! Loads and saves `config.toml`: the Workspace root, the branch template and
//! the Repo registry. Missing file or keys fall back to defaults.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Directory holding every Workspace. `None` only between parsing and
    /// `load` filling in the default, which needs `$HOME`.
    workspace_root: Option<PathBuf>,
    /// Branch name template; `{key}` is replaced with the Workspace name.
    #[serde(default = "default_branch_template")]
    pub branch_template: String,
    /// Registered Repos: name -> clone path. Sorted by name.
    #[serde(default)]
    repos: BTreeMap<String, PathBuf>,
}

fn default_branch_template() -> String {
    "{key}".to_string()
}

/// `$XDG_CONFIG_HOME/wrkt/config.toml`, else `$HOME/.config/wrkt/config.toml`.
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
    /// Load the config, or the defaults when the file doesn't exist.
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

    /// Write the config, creating its directory if needed.
    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let text = toml::to_string(self).context("serializing config")?;
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
    }

    /// Registered Repos as (name, clone path), sorted by name.
    pub fn repos(&self) -> impl Iterator<Item = (&str, &Path)> {
        self.repos.iter().map(|(n, p)| (n.as_str(), p.as_path()))
    }

    /// Register a Repo. Names must be usable as a directory name, and neither
    /// the name nor the clone may already be registered.
    pub fn add_repo(&mut self, name: &str, clone: &Path) -> Result<()> {
        if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
            bail!("invalid Repo name '{name}': it must be usable as a directory name");
        }
        if let Some(existing) = self.repos.get(name) {
            bail!(
                "a Repo named '{name}' is already registered ({})",
                existing.display()
            );
        }
        if let Some((other, _)) = self.repos.iter().find(|(_, p)| *p == clone) {
            bail!("{} is already registered as '{other}'", clone.display());
        }
        self.repos.insert(name.to_string(), clone.to_path_buf());
        Ok(())
    }

    /// Unregister a Repo.
    pub fn remove_repo(&mut self, name: &str) -> Result<()> {
        match self.repos.remove(name) {
            Some(_) => Ok(()),
            None => bail!("no Repo named '{name}' is registered"),
        }
    }
}
