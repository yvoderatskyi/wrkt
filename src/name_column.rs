use std::fmt;

pub struct NameColumn(usize);

impl NameColumn {
    pub fn fitting<'name, Name: AsRef<str> + 'name>(
        names: impl IntoIterator<Item = &'name Name>,
    ) -> Self {
        let width = names
            .into_iter()
            .map(|name| name.as_ref().chars().count())
            .max()
            .unwrap_or(0);
        NameColumn(width)
    }

    pub fn write_row(
        &self,
        formatter: &mut fmt::Formatter<'_>,
        name: &impl fmt::Display,
        rest: &impl fmt::Display,
    ) -> fmt::Result {
        writeln!(formatter, "{name:width$}  {rest}", width = self.0)
    }
}
