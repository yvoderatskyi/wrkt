# Coding standards

Reviewing Rust: load the `rust-skills` skill, read the rule files for every category the diff touches, and list the files you read in your report. The rules below, an ADR or the spec's Implementation Decisions win over a conflicting skill rule.

`scripts/check.sh` runs on every commit through `.githooks/pre-commit`. It enforces `cargo fmt`, clippy pedantic with warnings denied, and the rules marked _(checked)_ below. Spend review attention on the rest.

## Errors

Error handling uses `anyhow`: return `anyhow::Result`, attach `.context()` at fallible calls, and raise with `ensure!` for a failed condition check or `bail!` otherwise.

## Object Calisthenics

- One indentation level per function.
- No `else` _(checked for `if`/`else`; `let … else` is allowed)_.
- Primitives and strings wrapped in domain types.
- First-class collections.
- One dot per line (a fluent chain on one value such as an iterator or builder counts as one).
- No abbreviations.
- Small entities, at most two fields per struct.
- Behaviour instead of getters and setters.

## Self-explaining code

Code explains itself through names and types, with no comments or doc comments _(checked)_. The only comments are ones a tool or rule consumes: clap's `///` help text and the Arrange-Act-Assert markers below.

## Tests

- Tests follow Arrange-Act-Assert: three blocks headed `// Arrange`, `// Act` and `// Assert` and separated by blank lines, with one Act per test _(checked: exactly one of each marker, in order)_.
- Test names follow `<verb>_<what>_<when>`, e.g. `rejects_path_when_linked_worktree`; drop `_<when>` only when there is no condition. Tests sharing a subject (a command, the config) go in a `mod` named after it, so `<when>` names only the condition: `repo_add::rejects_path_when_linked_worktree`.
