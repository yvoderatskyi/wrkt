# wrkt

Manage a change that spans several git repos as one unit of work, with one command for all of them.

## Why

One Jira issue often needs changes in several repositories at once. By hand, that means repeating the same git steps in every repo: create a branch or worktree named after the issue, keep each one rebased onto `main`, force-push each one after a rebase, check which repos are dirty or unpushed, then clean everything up. wrkt does each of those steps once for all the repos and keeps the whole change in a single directory.

## Install

```sh
git clone https://github.com/yvoderatskyi/wrkt.git
cd wrkt
cargo install --path .
```

Requires Rust 1.85 or newer and `git` on your `PATH`. macOS is the only tested platform.

## Concepts

- **Repo**: a git repository registered with wrkt under a short name, so that it can join Workspaces.
- **Repo registry**: the set of Repos registered with wrkt, each under its short name.
- **Workspace**: one unit of work spanning one or more Repos, with one git worktree per Repo, all on the same branch and gathered under one directory.
- **Workspace root**: the directory that holds every Workspace, one subdirectory each.
- **Base branch**: the branch every Workspace's worktrees start from and are rebased onto; `main` for every Repo.
- **Branch template**: the pattern a Workspace's branch name is built from, with the Workspace name filled in.

## Walkthrough

Register each clone once. The Repo name defaults to the clone's directory name; pass `--name` to choose another.

```sh
wrkt repo add ~/code/api
wrkt repo add ~/code/web
```

Start work on an issue. `new` fetches each Repo and creates `~/wrkt/PROJ-123/api` and `~/wrkt/PROJ-123/web` as worktrees on the branch `PROJ-123`. If that branch already exists locally or on `origin`, wrkt resumes it instead of starting fresh, and says which it did.

```sh
wrkt new PROJ-123 api web
cd ~/wrkt/PROJ-123
```

In a terminal, leave out the name or the Repos and wrkt asks for them: `wrkt new` prompts for the name, then lets you pick Repos from the registry. `wrkt add` and `wrkt repo rm` prompt the same way. In a script or a pipe, a missing argument is an error instead.

The Workspace name is any string; a Jira issue key is just the usual choice, and wrkt never talks to Jira. Inside a Workspace's directory, every command works out the Workspace from where you are, so the rest of this walkthrough leaves the name out. From anywhere else, pass it: `wrkt sync PROJ-123`.

Commit in each worktree as usual with plain git. Then bring every Repo up to date with `origin/main` and publish:

```sh
wrkt sync   # fetch every Repo and rebase it onto origin/main
wrkt push   # push every Repo with commits ahead of origin/main, printing the host's PR links
```

Check where each Repo stands, without fetching:

```console
$ wrkt status
api  PROJ-123  origin/main +1 -0  origin/PROJ-123 +0 -0  clean
web  PROJ-123  origin/main +0 -0  not pushed  clean
```

Forgot a Repo? `wrkt add shared` brings it into the Workspace on the same branch. When the issue is merged, remove the worktrees, their local branches and the Workspace directory:

```sh
wrkt rm
```

## Commands

| Command | What it does |
| --- | --- |
| `wrkt repo add / list / rm` | Manage the Repo registry |
| `wrkt new` | Create a Workspace with one worktree per Repo |
| `wrkt add` | Add Repos to an existing Workspace |
| `wrkt list` | List every Workspace with its Repos |
| `wrkt status` | Show where each Repo in a Workspace stands, without fetching |
| `wrkt sync` | Fetch every Repo and rebase its worktree onto `origin/main` |
| `wrkt push` | Push every Repo with commits ahead of `origin/main` |
| `wrkt rm` | Remove every worktree, its local branch and the Workspace directory |

See `wrkt <command> --help` for arguments and flags.

## Configuration

wrkt reads `$XDG_CONFIG_HOME/wrkt/config.toml`, or `~/.config/wrkt/config.toml` when `XDG_CONFIG_HOME` is unset. Without a config file, wrkt starts with an empty Repo registry and the defaults below.

```toml
[workspace]
# Workspace root. Defaults to ~/wrkt. An absolute path, or one starting with ~/.
root = "/Users/you/work/issues"
# Branch template. {key} is replaced by the Workspace name. Defaults to "{key}".
# Override it for one Workspace with `wrkt new --branch`.
branch_template = "feature/{key}"

# The Repo registry. `wrkt repo add` and `wrkt repo rm` write this section for you.
[repos]
api = "/Users/you/code/api"
web = "/Users/you/code/web"
```

## What wrkt won't do to your work

- `sync` never stashes. It skips Repos with uncommitted changes or a rebase already in progress, and leaves a Repo that hits a conflict mid-rebase so you can finish with `git rebase --continue`. The other Repos still sync.
- `push` always uses `--force-with-lease`, so pushing after a rebase works but never overwrites commits someone else pushed to the branch.
- `rm` checks every Repo before removing anything and refuses if any has uncommitted changes or unpushed commits, naming which and why. `rm --force` removes it anyway.
- `rm` never touches remote branches, so open pull requests are unaffected.

Every multi-Repo command ends with one line per Repo and exits non-zero when any Repo conflicted, failed or blocked `rm`. When a Repo fails, its git output is printed below the summary; pass `-v` to stream every git command as it runs.

## Development

Run `git config core.hooksPath .githooks` once after cloning, so every commit runs `scripts/check.sh` (format, clippy, style rules, tests). `scripts/readme-demo.sh` replays the Walkthrough above in a throwaway sandbox, so you can refresh its output after a change. Coding conventions are in [CODING_STANDARDS.md](CODING_STANDARDS.md), the vocabulary in [GLOSSARY.md](GLOSSARY.md), and design decisions in [docs/adr/](docs/adr/).
