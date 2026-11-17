# Shell out to the git CLI instead of using a git library

wrkt runs every git operation by invoking the `git` binary rather than linking `git2` (libgit2) or `gitoxide`. libgit2's worktree and rebase support is thin, and push auth (SSH agent, credential helpers) plus hooks would become wrkt's problem. Calling `git` makes each operation behave exactly as it does when run by hand, which matters because the user resolves conflicts and inspects state with plain git afterwards.
