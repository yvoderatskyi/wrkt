use std::collections::BTreeMap;
use std::fmt;

use anyhow::{Error, Result, ensure};

use crate::name_column::NameColumn;
use crate::repo::RepoName;

pub trait Success {
    fn succeeded(&self) -> bool;
}

pub struct Report<Outcome>(BTreeMap<RepoName, Result<Outcome>>);

impl<Outcome> FromIterator<(RepoName, Result<Outcome>)> for Report<Outcome> {
    fn from_iter<Outcomes: IntoIterator<Item = (RepoName, Result<Outcome>)>>(
        outcomes: Outcomes,
    ) -> Self {
        Report(outcomes.into_iter().collect())
    }
}

impl<Outcome: Success> Report<Outcome> {
    pub fn succeeded(&self) -> bool {
        self.0.values().all(succeeded)
    }

    pub fn ensure_succeeded(&self) -> Result<()> {
        let unsuccessful = self
            .0
            .values()
            .filter(|outcome| !succeeded(outcome))
            .count();
        ensure!(
            self.succeeded(),
            "{unsuccessful} of {} Repos did not succeed",
            self.0.len()
        );
        Ok(())
    }
}

fn succeeded<Outcome: Success>(outcome: &Result<Outcome>) -> bool {
    outcome.as_ref().is_ok_and(Success::succeeded)
}

impl<Outcome> Report<Outcome> {
    fn failures(&self) -> impl Iterator<Item = (&RepoName, &Error)> {
        self.0
            .iter()
            .filter_map(|(name, outcome)| outcome.as_ref().err().map(|error| (name, error)))
    }
}

impl<Outcome: fmt::Display> fmt::Display for Report<Outcome> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let column = NameColumn::fitting(self.0.keys());
        self.0
            .iter()
            .try_for_each(|(name, outcome)| column.write_row(formatter, name, &Summary(outcome)))?;
        self.failures()
            .try_for_each(|(name, error)| write!(formatter, "\n{name}:\n{error:#}\n"))
    }
}

struct Summary<'outcome, Outcome>(&'outcome Result<Outcome>);

impl<Outcome: fmt::Display> fmt::Display for Summary<'_, Outcome> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Ok(outcome) => outcome.fmt(formatter),
            Err(_) => formatter.write_str("failed"),
        }
    }
}
