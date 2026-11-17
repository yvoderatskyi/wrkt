use std::str::FromStr;

use anyhow::{Error, Result, ensure};

use crate::git;

#[derive(Debug, Clone)]
pub struct Branch(String);

impl FromStr for Branch {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        name.to_string().try_into()
    }
}

impl TryFrom<String> for Branch {
    type Error = Error;

    fn try_from(name: String) -> Result<Self> {
        ensure!(
            git::is_valid_branch_name(&name)?,
            "invalid branch name '{name}'"
        );
        Ok(Branch(name))
    }
}

impl AsRef<str> for Branch {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
