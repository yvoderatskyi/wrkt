# wrkt

wrkt manages changes that span several git repos as one unit of work, typically one Jira issue.

## Language

**Workspace**:
One unit of work spanning one or more Repos, with one git worktree per Repo, all on the same branch and gathered under one directory.
_Avoid_: Task, session, feature, stack

**Repo**:
A git repository registered with wrkt under a short name, so that it can join Workspaces.
_Avoid_: Project, module

**Workspace root**:
The directory that holds every Workspace, one subdirectory each.
_Avoid_: Worktree dir, base dir

**Base branch**:
The branch every Workspace's worktrees start from and are rebased onto; `main` for every Repo.
_Avoid_: Trunk, parent branch, upstream

**Repo registry**:
The set of Repos registered with wrkt, each under its short name.
_Avoid_: Repo list, repo config

**Branch template**:
The pattern a Workspace's branch name is built from, with the Workspace name filled in.
_Avoid_: Branch pattern, naming scheme
