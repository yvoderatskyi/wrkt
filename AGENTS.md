# wrkt

A git worktree manager for multiple repos at once, written in Rust.

## Agent skills

### Issue tracker

Issues and specs live as local markdown files under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md` for creating, fetching and closing them.

### Triage labels

The five default triage labels plus `done`, recorded on each issue file's `Status:` line. See `docs/agents/triage-labels.md`.

### Rust

Standards for writing and reviewing Rust live in `CODING_STANDARDS.md`. Run `scripts/check.sh` before committing; the pre-commit hook runs it too.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
