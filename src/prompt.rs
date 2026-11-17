use std::fmt;
use std::io::IsTerminal;
use std::process::ExitCode;

use anyhow::{Result, bail, ensure};
use inquire::list_option::ListOption;
use inquire::validator::{ErrorMessage, Validation};
use inquire::{CustomUserError, InquireError, MultiSelect, Select, Text};

use crate::repo::{Registry, RepoName, RepoNames};
use crate::workspace::WorkspaceName;

const INTERRUPTED: u8 = 130;

pub trait Prompter {
    fn workspace_name(&mut self) -> Result<WorkspaceName>;
    fn repos(&mut self, choices: RepoNames) -> Result<Vec<RepoName>>;
    fn repo(&mut self, choices: RepoNames) -> Result<RepoName>;
}

pub fn for_this_terminal() -> Box<dyn Prompter> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        return Box::new(Terminal);
    }
    Box::new(NonInteractive)
}

struct NonInteractive;

impl Prompter for NonInteractive {
    fn workspace_name(&mut self) -> Result<WorkspaceName> {
        bail!("no Workspace name given")
    }

    fn repos(&mut self, _choices: RepoNames) -> Result<Vec<RepoName>> {
        bail!("no Repo given")
    }

    fn repo(&mut self, _choices: RepoNames) -> Result<RepoName> {
        bail!("no Repo given")
    }
}

struct Terminal;

impl Prompter for Terminal {
    fn workspace_name(&mut self) -> Result<WorkspaceName> {
        let name = Text::new("Workspace name:")
            .with_validator(usable_workspace_name)
            .prompt()
            .map_err(cancelled)?;
        name.parse()
    }

    fn repos(&mut self, choices: RepoNames) -> Result<Vec<RepoName>> {
        MultiSelect::new("Repos:", choices.into_iter().collect())
            .with_validator(at_least_one_repo)
            .prompt()
            .map_err(cancelled)
    }

    fn repo(&mut self, choices: RepoNames) -> Result<RepoName> {
        Select::new("Repo:", choices.into_iter().collect())
            .prompt()
            .map_err(cancelled)
    }
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "inquire validators must return a Result"
)]
fn usable_workspace_name(input: &str) -> Result<Validation, CustomUserError> {
    let Err(error) = input.parse::<WorkspaceName>() else {
        return Ok(Validation::Valid);
    };
    Ok(Validation::Invalid(ErrorMessage::Custom(error.to_string())))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "inquire validators must return a Result"
)]
fn at_least_one_repo(selected: &[ListOption<&RepoName>]) -> Result<Validation, CustomUserError> {
    if selected.is_empty() {
        return Ok(Validation::Invalid("select at least one Repo".into()));
    }
    Ok(Validation::Valid)
}

fn cancelled(error: InquireError) -> anyhow::Error {
    if matches!(
        error,
        InquireError::OperationCanceled | InquireError::OperationInterrupted
    ) {
        return anyhow::Error::new(Cancelled);
    }
    anyhow::Error::new(error).context("prompting")
}

pub fn new_workspace_arguments(
    name: Option<WorkspaceName>,
    repos: Vec<RepoName>,
    registry: &Registry,
    prompter: &mut dyn Prompter,
) -> Result<(WorkspaceName, Vec<RepoName>)> {
    if repos.is_empty() {
        registered(registry)?;
    }
    let name = name.map_or_else(|| prompter.workspace_name(), Ok)?;
    let repos = given_or_chosen(repos, || registered(registry), prompter)?;
    Ok((name, repos))
}

pub fn added_repos(
    given: Vec<RepoName>,
    workspace: &WorkspaceName,
    members: &RepoNames,
    registry: &Registry,
    prompter: &mut dyn Prompter,
) -> Result<Vec<RepoName>> {
    let outside = || {
        let choices = registry.names().without(members);
        ensure!(
            !choices.is_empty(),
            "every registered Repo is already in {workspace}"
        );
        Ok(choices)
    };
    given_or_chosen(given, outside, prompter)
}

pub fn removed_repo(
    given: Option<RepoName>,
    registry: &Registry,
    prompter: &mut dyn Prompter,
) -> Result<RepoName> {
    given.map_or_else(|| prompter.repo(registered(registry)?), Ok)
}

fn given_or_chosen(
    given: Vec<RepoName>,
    choices: impl FnOnce() -> Result<RepoNames>,
    prompter: &mut dyn Prompter,
) -> Result<Vec<RepoName>> {
    if !given.is_empty() {
        return Ok(given);
    }
    prompter.repos(choices()?)
}

fn registered(registry: &Registry) -> Result<RepoNames> {
    let names = registry.names();
    ensure!(
        !names.is_empty(),
        "no Repos registered; run `wrkt repo add <path>`"
    );
    Ok(names)
}

#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("cancelled")
    }
}

impl std::error::Error for Cancelled {}

pub fn exit_code(error: &anyhow::Error) -> ExitCode {
    if error.is::<Cancelled>() {
        return ExitCode::from(INTERRUPTED);
    }
    ExitCode::FAILURE
}

#[cfg(test)]
mod scripted {
    use anyhow::Result;

    use super::{Cancelled, Prompter};
    use crate::repo::{RepoName, RepoNames};
    use crate::workspace::WorkspaceName;

    pub struct Scripted {
        answer: Option<&'static str>,
        offered: RepoNames,
    }

    impl Scripted {
        pub fn answering(answer: &'static str) -> Self {
            Scripted {
                answer: Some(answer),
                offered: RepoNames::default(),
            }
        }

        pub fn cancelling() -> Self {
            Scripted {
                answer: None,
                offered: RepoNames::default(),
            }
        }

        pub fn offered(&self) -> String {
            self.offered.to_string()
        }

        fn answer(&self) -> Result<&'static str> {
            self.answer.ok_or_else(|| anyhow::Error::new(Cancelled))
        }
    }

    pub fn joined(names: &[RepoName]) -> String {
        let names: Vec<_> = names.iter().map(RepoName::to_string).collect();
        names.join(" ")
    }

    impl Prompter for Scripted {
        fn workspace_name(&mut self) -> Result<WorkspaceName> {
            self.answer()?.parse()
        }

        fn repos(&mut self, choices: RepoNames) -> Result<Vec<RepoName>> {
            self.offered = choices;
            self.answer()?.split(' ').map(str::parse).collect()
        }

        fn repo(&mut self, choices: RepoNames) -> Result<RepoName> {
            self.offered = choices;
            self.answer()?.parse()
        }
    }
}

#[cfg(test)]
mod new_workspace_arguments {
    use super::scripted::{Scripted, joined};
    use super::{Cancelled, new_workspace_arguments};
    use crate::repo::Registry;

    fn registry() -> Registry {
        toml::from_str("api = \"/clones/api\"\nweb = \"/clones/web\"").unwrap()
    }

    #[test]
    fn asks_prompter_when_name_omitted() {
        // Arrange
        let mut prompter = Scripted::answering("PROJ-123");

        // Act
        let (name, _) = new_workspace_arguments(
            None,
            vec!["api".parse().unwrap()],
            &registry(),
            &mut prompter,
        )
        .unwrap();

        // Assert
        assert_eq!(name.to_string(), "PROJ-123");
    }

    #[test]
    fn offers_every_registered_repo_when_repos_omitted() {
        // Arrange
        let mut prompter = Scripted::answering("web");

        // Act
        let (_, repos) = new_workspace_arguments(
            Some("PROJ-123".parse().unwrap()),
            Vec::new(),
            &registry(),
            &mut prompter,
        )
        .unwrap();

        // Assert
        assert_eq!(prompter.offered(), "api web");
        assert_eq!(joined(&repos), "web");
    }

    #[test]
    fn rejects_omitted_repos_before_asking_name_when_registry_empty() {
        // Arrange
        let registry = Registry::default();
        let mut prompter = Scripted::cancelling();

        // Act
        let error =
            new_workspace_arguments(None, Vec::new(), &registry, &mut prompter).unwrap_err();

        // Assert
        assert_eq!(
            error.to_string(),
            "no Repos registered; run `wrkt repo add <path>`"
        );
    }

    #[test]
    fn stops_with_cancelled_when_name_prompt_cancelled() {
        // Arrange
        let mut prompter = Scripted::cancelling();

        // Act
        let error =
            new_workspace_arguments(None, Vec::new(), &registry(), &mut prompter).unwrap_err();

        // Assert
        assert!(error.is::<Cancelled>(), "not cancelled: {error:#}");
    }
}

#[cfg(test)]
mod added_repos {
    use super::added_repos;
    use super::scripted::{Scripted, joined};
    use crate::repo::{Registry, RepoNames};
    use crate::workspace::WorkspaceName;

    fn registry() -> Registry {
        toml::from_str("api = \"/clones/api\"\nshared = \"/clones/shared\"\nweb = \"/clones/web\"")
            .unwrap()
    }

    fn workspace() -> WorkspaceName {
        "PROJ-123".parse().unwrap()
    }

    #[test]
    fn offers_repos_outside_workspace_when_repos_omitted() {
        // Arrange
        let members: RepoNames = std::iter::once("api".parse().unwrap()).collect();
        let mut prompter = Scripted::answering("web");

        // Act
        let repos = added_repos(
            Vec::new(),
            &workspace(),
            &members,
            &registry(),
            &mut prompter,
        )
        .unwrap();

        // Assert
        assert_eq!(prompter.offered(), "shared web");
        assert_eq!(joined(&repos), "web");
    }

    #[test]
    fn rejects_omitted_repos_when_every_repo_already_member() {
        // Arrange
        let members = registry().names();
        let mut prompter = Scripted::answering("web");

        // Act
        let error = added_repos(
            Vec::new(),
            &workspace(),
            &members,
            &registry(),
            &mut prompter,
        )
        .unwrap_err();

        // Assert
        assert_eq!(
            error.to_string(),
            "every registered Repo is already in PROJ-123"
        );
    }
}

#[cfg(test)]
mod removed_repo {
    use super::removed_repo;
    use super::scripted::Scripted;
    use crate::repo::Registry;

    #[test]
    fn offers_every_registered_repo_when_name_omitted() {
        // Arrange
        let registry: Registry =
            toml::from_str("api = \"/clones/api\"\nweb = \"/clones/web\"").unwrap();
        let mut prompter = Scripted::answering("web");

        // Act
        let repo = removed_repo(None, &registry, &mut prompter).unwrap();

        // Assert
        assert_eq!(prompter.offered(), "api web");
        assert_eq!(repo.to_string(), "web");
    }

    #[test]
    fn rejects_omitted_name_when_registry_empty() {
        // Arrange
        let registry = Registry::default();
        let mut prompter = Scripted::answering("api");

        // Act
        let error = removed_repo(None, &registry, &mut prompter).unwrap_err();

        // Assert
        assert_eq!(
            error.to_string(),
            "no Repos registered; run `wrkt repo add <path>`"
        );
    }
}

#[cfg(test)]
mod exit_code {
    use std::process::ExitCode;

    use super::{Cancelled, exit_code};

    #[test]
    fn exits_130_when_prompt_cancelled() {
        // Arrange
        let error = anyhow::Error::new(Cancelled);

        // Act
        let code = exit_code(&error);

        // Assert
        assert_eq!(code, ExitCode::from(130));
    }

    #[test]
    fn exits_1_when_command_failed() {
        // Arrange
        let error = anyhow::anyhow!("no Repo given");

        // Act
        let code = exit_code(&error);

        // Assert
        assert_eq!(code, ExitCode::FAILURE);
    }
}
