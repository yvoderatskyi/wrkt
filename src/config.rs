//! Loads and saves `config.toml`: the Workspace root, the branch template and
//! the Repo registry. Missing file or keys fall back to defaults.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_BRANCH_TEMPLATE: &str = "{key}";

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Directory holding every Workspace.
    pub workspace_root: PathBuf,
    /// Branch name template; `{key}` is replaced with the Workspace name.
    #[serde(default = "default_branch_template")]
    pub branch_template: String,
    /// Registered Repos: name -> clone path. Sorted by name.
    #[serde(default)]
    pub repos: BTreeMap<String, PathBuf>,
}

fn default_branch_template() -> String {
    DEFAULT_BRANCH_TEMPLATE.to_string()
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
    fn default_for_home(home: &Path) -> Self {
        Config {
            workspace_root: home.join("wrkt"),
            branch_template: default_branch_template(),
            repos: BTreeMap::new(),
        }
    }

    /// Load the config, or the defaults when the file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                #[derive(Deserialize)]
                struct Partial {
                    workspace_root: Option<PathBuf>,
                    #[serde(default = "default_branch_template")]
                    branch_template: String,
                    #[serde(default)]
                    repos: BTreeMap<String, PathBuf>,
                }
                let p: Partial = toml::from_str(&text)
                    .with_context(|| format!("invalid config {}", path.display()))?;
                let workspace_root = match p.workspace_root {
                    Some(root) => root,
                    None => Self::default_for_home(&home()?).workspace_root,
                };
                Ok(Config {
                    workspace_root,
                    branch_template: p.branch_template,
                    repos: p.repos,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(Self::default_for_home(&home()?))
            }
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
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
}
