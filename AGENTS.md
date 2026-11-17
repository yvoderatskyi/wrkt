# wrkt

A git worktree manager for multiple repos at once, written in Rust.

## Agent skills

### Issue tracker

Issues and specs live as local markdown files under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

The five default triage labels, recorded on each issue file's `Status:` line. See `docs/agents/triage-labels.md`.

### Rust

Writing, refactoring or reviewing Rust: load the `rust-skills` skill and apply its rules to the code you touch. The rules below, an ADR or the spec's Implementation Decisions win over a conflicting skill rule.

- Error handling uses `anyhow`: return `anyhow::Result`, attach `.context()` at fallible calls, and raise with `ensure!` for a failed condition check or `bail!` otherwise.
- Code explains itself through names and types, with no comments or doc comments. The only comments are ones a tool or rule consumes: clap's `///` help text and the Arrange-Act-Assert markers below.
- Tests follow Arrange-Act-Assert: three blocks headed `// Arrange`, `// Act` and `// Assert` and separated by blank lines, with one Act per test.
- Test names follow `<verb>_<what>_<when>`, e.g. `rejects_path_when_linked_worktree`; drop `_<when>` only when there is no condition. Tests sharing a subject (a command, the config) go in a `mod` named after it, so `<when>` names only the condition: `repo_add::rejects_path_when_linked_worktree`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
