mod support;

mod new {
    use super::support::Sandbox;

    #[test]
    fn creates_worktree_on_key_branch_from_origin_main_when_single_repo() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Assert
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        assert_eq!(
            sandbox.git(&worktree, &["branch", "--show-current"]),
            "PROJ-123"
        );
        assert_eq!(
            sandbox.git(&worktree, &["rev-parse", "HEAD"]),
            sandbox.git(&api, &["rev-parse", "origin/main"])
        );
        assert_eq!(
            sandbox.git(
                &worktree,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"]
            ),
            api.join(".git").to_str().unwrap()
        );
    }

    #[test]
    fn creates_worktrees_on_same_branch_when_several_repos() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.registered_repo("web");

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api", "web"]).success();

        // Assert
        let branches: Vec<_> = ["api", "web"]
            .map(|repo| sandbox.workspace_root().join("PROJ-123").join(repo))
            .iter()
            .map(|worktree| sandbox.git(worktree, &["branch", "--show-current"]))
            .collect();
        assert_eq!(branches, ["PROJ-123", "PROJ-123"]);
    }

    #[test]
    fn starts_branch_from_latest_origin_main_when_clone_is_stale() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        let latest = sandbox.advance_origin_main("api");

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Assert
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        assert_eq!(sandbox.git(&worktree, &["rev-parse", "HEAD"]), latest);
    }

    #[test]
    fn names_branch_from_template_when_configured() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config("branch_template = \"feature/{key}-work\"\n");
        sandbox.registered_repo("api");

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Assert
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        assert_eq!(
            sandbox.git(&worktree, &["branch", "--show-current"]),
            "feature/PROJ-123-work"
        );
    }

    #[test]
    fn names_branch_from_flag_when_branch_given() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config("branch_template = \"feature/{key}\"\n");
        sandbox.registered_repo("api");

        // Act
        sandbox
            .wrkt(&["new", "PROJ-123", "api", "--branch", "hotfix/login"])
            .success();

        // Assert
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        assert_eq!(
            sandbox.git(&worktree, &["branch", "--show-current"]),
            "hotfix/login"
        );
    }

    #[test]
    fn reuses_local_branch_when_it_exists() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        let local_work = sandbox.commit_on_local_branch(&api, "PROJ-123");

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Assert
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        assert_eq!(
            sandbox.git(&worktree, &["branch", "--show-current"]),
            "PROJ-123"
        );
        assert_eq!(sandbox.git(&worktree, &["rev-parse", "HEAD"]), local_work);
    }

    #[test]
    fn tracks_remote_branch_when_no_local_branch() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        let remote_work = sandbox.push_colleague_commit("api", "PROJ-123");

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Assert
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        assert_eq!(sandbox.git(&worktree, &["rev-parse", "HEAD"]), remote_work);
        let upstream = sandbox.git(
            &api,
            &[
                "for-each-ref",
                "--format=%(upstream)",
                "refs/heads/PROJ-123",
            ],
        );
        assert_eq!(upstream, "refs/remotes/origin/PROJ-123");
    }

    #[test]
    fn names_resolution_per_repo_when_cases_differ() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.registered_repo("web");
        sandbox.registered_repo("shared");
        sandbox.commit_on_local_branch(&api, "PROJ-123");
        sandbox.push_colleague_commit("web", "PROJ-123");

        // Act
        let run = sandbox.wrkt(&["new", "PROJ-123", "api", "web", "shared"]);

        // Assert
        let output = run.success().stdout();
        let expected = [
            ("api", "reused local branch PROJ-123"),
            ("web", "tracking origin/PROJ-123"),
            ("shared", "created PROJ-123 from origin/main"),
        ];
        let missing: Vec<_> = expected
            .iter()
            .filter(|(repo, case)| {
                !output
                    .lines()
                    .any(|line| line.starts_with(repo) && line.contains(case))
            })
            .collect();
        assert!(missing.is_empty(), "missing {missing:?} in:\n{output}");
    }

    #[test]
    fn creates_branch_from_origin_main_when_remote_branch_deleted() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.push_colleague_commit("api", "PROJ-123");
        sandbox.git(&api, &["fetch", "origin"]);
        sandbox.delete_origin_branch("api", "PROJ-123");

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Assert
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        assert_eq!(
            sandbox.git(&worktree, &["rev-parse", "HEAD"]),
            sandbox.git(&api, &["rev-parse", "refs/remotes/origin/main"])
        );
    }

    #[test]
    fn creates_branch_from_origin_main_when_local_branch_shadows_it() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.git(&api, &["branch", "origin/main"]);
        let latest = sandbox.advance_origin_main("api");

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Assert
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        assert_eq!(sandbox.git(&worktree, &["rev-parse", "HEAD"]), latest);
    }

    #[test]
    fn names_repo_when_its_worktree_fails() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        let web = sandbox.registered_repo("web");
        sandbox.git(&web, &["switch", "-c", "PROJ-123"]);

        // Act
        let run = sandbox.wrkt(&["new", "PROJ-123", "api", "web"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("adding a worktree for Repo 'web'"),
            "stderr: {}",
            run.stderr()
        );
    }

    #[test]
    fn names_repo_when_its_fetch_fails() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        let web = sandbox.registered_repo("web");
        sandbox.break_origin(&web);

        // Act
        let run = sandbox.wrkt(&["new", "PROJ-123", "api", "web"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("fetching Repo 'web'"),
            "stderr: {}",
            run.stderr()
        );
    }

    #[test]
    fn rejects_workspace_when_already_exists() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        let web = sandbox.registered_repo("web");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["new", "PROJ-123", "web"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("already exists"),
            "stderr: {}",
            run.stderr()
        );
        assert!(!sandbox.workspace_root().join("PROJ-123/web").exists());
        assert_eq!(sandbox.git(&web, &["branch", "--list", "PROJ-123"]), "");
    }

    #[test]
    fn rejects_repo_when_not_registered() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");

        // Act
        let run = sandbox.wrkt(&["new", "PROJ-123", "api", "nope"]);

        // Assert
        run.failure();
        assert!(run.stderr().contains("nope"), "stderr: {}", run.stderr());
        assert!(!sandbox.workspace_root().join("PROJ-123").exists());
        assert_eq!(sandbox.git(&api, &["branch", "--list", "PROJ-123"]), "");
    }

    #[test]
    fn rejects_repo_when_named_twice() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");

        // Act
        let run = sandbox.wrkt(&["new", "PROJ-123", "api", "api"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("more than once"),
            "stderr: {}",
            run.stderr()
        );
        assert!(!sandbox.workspace_root().join("PROJ-123").exists());
        assert_eq!(sandbox.git(&api, &["branch", "--list", "PROJ-123"]), "");
    }

    #[test]
    fn rejects_name_when_unusable_as_directory() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");

        // Act
        let attempts: Vec<_> = ["", "a/b", ".", ".."]
            .into_iter()
            .map(|name| {
                (
                    name,
                    sandbox.wrkt(&["new", name, "api", "--branch", "safe"]),
                )
            })
            .collect();

        // Assert
        let accepted: Vec<_> = attempts
            .iter()
            .filter(|(_, run)| run.succeeded() || !run.stderr().contains("invalid Workspace name"))
            .map(|(name, _)| *name)
            .collect();
        assert!(accepted.is_empty(), "not rejected as invalid: {accepted:?}");
        assert_eq!(sandbox.git(&api, &["worktree", "list"]).lines().count(), 1);
    }

    #[test]
    fn rejects_branch_when_git_refuses_name() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");

        // Act
        let run = sandbox.wrkt(&["new", "PROJ-123", "api", "--branch", "bad..name"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("invalid branch name 'bad..name'"),
            "stderr: {}",
            run.stderr()
        );
        assert!(!sandbox.workspace_root().join("PROJ-123").exists());
        assert_eq!(sandbox.git(&api, &["worktree", "list"]).lines().count(), 1);
    }

    #[test]
    fn leaves_branch_without_upstream() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Assert
        let upstream = sandbox.git(
            &api,
            &[
                "for-each-ref",
                "--format=%(upstream)",
                "refs/heads/PROJ-123",
            ],
        );
        assert_eq!(upstream, "");
    }

    #[test]
    fn creates_nothing_when_a_later_repo_fails_to_fetch() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        let web = sandbox.registered_repo("web");
        sandbox.break_origin(&web);

        // Act
        let run = sandbox.wrkt(&["new", "PROJ-123", "api", "web"]);

        // Assert
        run.failure();
        assert!(!sandbox.workspace_root().join("PROJ-123").exists());
        assert_eq!(sandbox.git(&api, &["branch", "--list", "PROJ-123"]), "");
    }
}

mod list {
    use super::support::Sandbox;

    #[test]
    fn shows_each_workspace_with_its_repos() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.registered_repo("web");
        sandbox.wrkt(&["new", "PROJ-2", "web", "api"]).success();
        sandbox.wrkt(&["new", "PROJ-1", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["list"]);

        // Assert
        let listing = run.success().stdout();
        let rows: Vec<Vec<_>> = listing
            .lines()
            .map(|line| line.split_whitespace().collect())
            .collect();
        assert_eq!(rows, [vec!["PROJ-1", "api"], vec!["PROJ-2", "api", "web"]]);
    }

    #[test]
    fn shows_nothing_when_workspace_root_missing() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");

        // Act
        let run = sandbox.wrkt(&["list"]);

        // Assert
        assert_eq!(run.success().stdout(), "");
    }

    #[test]
    fn ignores_children_when_not_worktrees_of_registered_repos() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        let unregistered = sandbox.repo("other");
        sandbox.wrkt(&["new", "PROJ-1", "api"]).success();
        let workspace = sandbox.workspace_root().join("PROJ-1");
        std::fs::create_dir_all(workspace.join("notes")).unwrap();
        sandbox.git(
            &unregistered,
            &[
                "worktree",
                "add",
                "-b",
                "PROJ-1",
                workspace.join("other").to_str().unwrap(),
            ],
        );

        // Act
        let run = sandbox.wrkt(&["list"]);

        // Assert
        assert_eq!(
            run.success()
                .stdout()
                .split_whitespace()
                .collect::<Vec<_>>(),
            ["PROJ-1", "api"]
        );
    }

    #[test]
    fn shows_repos_when_workspace_root_is_a_symlink() {
        // Arrange
        let sandbox = Sandbox::new();
        let link = sandbox.home().join("workspaces-link");
        std::fs::create_dir_all(sandbox.workspace_root()).unwrap();
        std::os::unix::fs::symlink(sandbox.workspace_root(), &link).unwrap();
        sandbox.write_config_rooted_at(&link, "");
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-1", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["list"]);

        // Assert
        assert_eq!(
            run.success()
                .stdout()
                .split_whitespace()
                .collect::<Vec<_>>(),
            ["PROJ-1", "api"]
        );
    }
}

mod which {
    use super::support::Sandbox;

    #[test]
    fn infers_workspace_when_inside_its_subdirectory() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let nested = sandbox.workspace_root().join("PROJ-123/api/src");
        std::fs::create_dir_all(&nested).unwrap();

        // Act
        let run = sandbox.wrkt_in(&nested, &["which"]);

        // Assert
        assert_eq!(run.success().stdout(), "PROJ-123\n");
    }

    #[test]
    fn rejects_omitted_name_when_outside_workspace_root() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt_in(&sandbox.home(), &["which"]);

        // Assert
        run.failure();
        assert_eq!(
            run.stderr(),
            format!(
                "error: {} is not inside a Workspace under {}; pass a Workspace name\n",
                sandbox.home().display(),
                sandbox.workspace_root().display()
            )
        );
    }

    #[test]
    fn rejects_omitted_name_when_at_workspace_root() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt_in(&sandbox.workspace_root(), &["which"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("not inside a Workspace")
                && run.stderr().contains("pass a Workspace name"),
            "stderr: {}",
            run.stderr()
        );
    }

    #[test]
    fn rejects_omitted_name_when_in_sibling_sharing_root_prefix() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let mut sibling = sandbox.workspace_root().into_os_string();
        sibling.push("-other/PROJ-123");
        std::fs::create_dir_all(&sibling).unwrap();

        // Act
        let run = sandbox.wrkt_in(sibling.as_ref(), &["which"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("not inside a Workspace")
                && run.stderr().contains("pass a Workspace name"),
            "stderr: {}",
            run.stderr()
        );
    }

    #[test]
    fn prefers_explicit_name_when_inside_another_workspace() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-1", "api"]).success();
        let inside = sandbox.workspace_root().join("PROJ-1/api");

        // Act
        let run = sandbox.wrkt_in(&inside, &["which", "PROJ-2"]);

        // Assert
        assert_eq!(run.success().stdout(), "PROJ-2\n");
    }

    #[test]
    fn infers_workspace_when_root_is_a_symlink() {
        // Arrange
        let sandbox = Sandbox::new();
        let link = sandbox.home().join("workspaces-link");
        std::fs::create_dir_all(sandbox.workspace_root()).unwrap();
        std::os::unix::fs::symlink(sandbox.workspace_root(), &link).unwrap();
        sandbox.write_config_rooted_at(&link, "");
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt_in(&link.join("PROJ-123/api"), &["which"]);

        // Assert
        assert_eq!(run.success().stdout(), "PROJ-123\n");
    }
}

mod add {
    use super::support::Sandbox;

    #[test]
    fn adds_worktree_on_workspace_branch_when_name_given() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        let shared = sandbox.registered_repo("shared");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["add", "PROJ-123", "shared"]);

        // Assert
        run.success();
        let worktree = sandbox.workspace_root().join("PROJ-123/shared");
        assert_eq!(
            sandbox.git(&worktree, &["branch", "--show-current"]),
            "PROJ-123"
        );
        assert_eq!(
            sandbox.git(&worktree, &["rev-parse", "HEAD"]),
            sandbox.git(&shared, &["rev-parse", "origin/main"])
        );
    }

    #[test]
    fn uses_branch_of_members_when_workspace_created_with_branch_flag() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.registered_repo("shared");
        sandbox
            .wrkt(&["new", "PROJ-123", "api", "--branch", "feature/custom"])
            .success();

        // Act
        let run = sandbox.wrkt(&["add", "PROJ-123", "shared"]);

        // Assert
        run.success();
        let worktree = sandbox.workspace_root().join("PROJ-123/shared");
        assert_eq!(
            sandbox.git(&worktree, &["branch", "--show-current"]),
            "feature/custom"
        );
    }

    #[test]
    fn reuses_local_branch_when_it_exists() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        let shared = sandbox.registered_repo("shared");
        let local_work = sandbox.commit_on_local_branch(&shared, "PROJ-123");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["add", "PROJ-123", "shared"]);

        // Assert
        assert!(
            run.success()
                .stdout()
                .contains("reused local branch PROJ-123")
        );
        let worktree = sandbox.workspace_root().join("PROJ-123/shared");
        assert_eq!(sandbox.git(&worktree, &["rev-parse", "HEAD"]), local_work);
    }

    #[test]
    fn tracks_remote_branch_when_pushed_after_workspace_created() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.registered_repo("shared");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let remote_work = sandbox.push_colleague_commit("shared", "PROJ-123");

        // Act
        let run = sandbox.wrkt(&["add", "PROJ-123", "shared"]);

        // Assert
        assert!(run.success().stdout().contains("tracking origin/PROJ-123"));
        let worktree = sandbox.workspace_root().join("PROJ-123/shared");
        assert_eq!(sandbox.git(&worktree, &["rev-parse", "HEAD"]), remote_work);
    }

    #[test]
    fn rejects_workspace_when_missing() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.registered_repo("shared");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["add", "PROJ-999", "shared"]);

        // Assert
        assert!(
            run.failure()
                .stderr()
                .contains("no Workspace named 'PROJ-999'")
        );
        assert!(!sandbox.workspace_root().join("PROJ-999").exists());
    }

    #[test]
    fn rejects_repo_when_not_registered() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["add", "PROJ-123", "shared"]);

        // Assert
        assert!(
            run.failure()
                .stderr()
                .contains("no Repo named 'shared' is registered")
        );
        assert!(!sandbox.workspace_root().join("PROJ-123/shared").exists());
    }

    #[test]
    fn rejects_repo_when_already_member() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        sandbox.break_origin(&api);

        // Act
        let run = sandbox.wrkt(&["add", "PROJ-123", "api"]);

        // Assert
        assert!(
            run.failure()
                .stderr()
                .contains("Repo 'api' is already in Workspace")
        );
    }

    #[test]
    fn infers_workspace_when_name_omitted_inside_it() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.registered_repo("shared");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let inside = sandbox.workspace_root().join("PROJ-123/api");

        // Act
        let run = sandbox.wrkt_in(&inside, &["add", "shared"]);

        // Assert
        run.success();
        let worktree = sandbox.workspace_root().join("PROJ-123/shared");
        assert_eq!(
            sandbox.git(&worktree, &["branch", "--show-current"]),
            "PROJ-123"
        );
    }

    #[test]
    fn adds_every_repo_when_name_omitted_and_several_repos_given() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.registered_repo("shared");
        sandbox.registered_repo("web");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let inside = sandbox.workspace_root().join("PROJ-123/api");

        // Act
        let run = sandbox.wrkt_in(&inside, &["add", "shared", "web"]);

        // Assert
        run.success();
        let branches: Vec<_> = ["shared", "web"]
            .map(|repo| sandbox.workspace_root().join("PROJ-123").join(repo))
            .iter()
            .map(|worktree| sandbox.git(worktree, &["branch", "--show-current"]))
            .collect();
        assert_eq!(branches, ["PROJ-123", "PROJ-123"]);
    }

    #[test]
    fn rejects_omitted_name_when_outside_workspace_root() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.registered_repo("shared");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["add", "shared"]);

        // Assert
        assert!(run.failure().stderr().contains("pass a Workspace name"));
    }

    #[test]
    fn uses_template_branch_when_no_member_is_on_a_branch() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config("branch_template = \"feature/{key}\"\n");
        sandbox.registered_repo("shared");
        std::fs::create_dir_all(sandbox.workspace_root().join("PROJ-123")).unwrap();

        // Act
        let run = sandbox.wrkt(&["add", "PROJ-123", "shared"]);

        // Assert
        run.success();
        let worktree = sandbox.workspace_root().join("PROJ-123/shared");
        assert_eq!(
            sandbox.git(&worktree, &["branch", "--show-current"]),
            "feature/PROJ-123"
        );
    }
}

mod status {
    use super::support::Sandbox;

    #[test]
    fn reports_clean_branch_level_with_base_when_workspace_fresh() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("PROJ-123"), "{line}");
        assert!(line.contains("clean"), "{line}");
        assert!(line.contains("origin/main +0 -0"), "{line}");
        assert!(line.contains("not pushed"), "{line}");
    }

    #[test]
    fn reports_dirty_when_worktree_has_uncommitted_changes() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        std::fs::write(worktree.join("README.md"), "edited\n").unwrap();

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("dirty"), "{line}");
    }

    #[test]
    fn counts_commits_ahead_and_behind_origin_main_when_both_moved() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        sandbox.commit_in(&worktree, "ONE.md");
        sandbox.commit_in(&worktree, "TWO.md");
        sandbox.advance_origin_main("api");
        sandbox.git(&api, &["fetch", "origin"]);

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("origin/main +2 -1"), "{line}");
    }

    #[test]
    fn reads_local_state_without_fetching() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        sandbox.advance_origin_main("api");

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("origin/main +0 -0"), "{line}");
    }

    #[test]
    fn counts_commits_ahead_and_behind_remote_branch_when_pushed() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        sandbox.git(&worktree, &["push", "origin", "PROJ-123"]);
        sandbox.push_colleague_commit("api", "PROJ-123");
        sandbox.git(&api, &["fetch", "origin"]);
        sandbox.commit_in(&worktree, "ONE.md");
        sandbox.commit_in(&worktree, "TWO.md");

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("origin/PROJ-123 +2 -1"), "{line}");
        assert!(!line.contains("not pushed"), "{line}");
    }

    #[test]
    fn reports_rebase_on_workspace_branch_when_rebase_in_progress() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        sandbox.rebase_into_conflict("api", &api, &worktree);

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("rebasing"), "{line}");
        assert!(line.contains("PROJ-123"), "{line}");
    }

    #[test]
    fn counts_from_branch_tip_when_rebase_in_progress() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        sandbox.rebase_into_conflict("api", &api, &worktree);

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("origin/main +1 -1"), "{line}");
    }

    #[test]
    fn omits_rebase_when_none_in_progress() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(!line.contains("rebasing"), "{line}");
    }

    #[test]
    fn sorts_repos_by_name_when_created_out_of_order() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("web");
        sandbox.registered_repo("api");
        sandbox.registered_repo("shared");
        sandbox
            .wrkt(&["new", "PROJ-123", "web", "shared", "api"])
            .success();

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        let stdout = run.success().stdout();
        let repos: Vec<_> = stdout
            .lines()
            .filter_map(|line| line.split_whitespace().next())
            .collect();
        assert_eq!(repos, ["api", "shared", "web"]);
    }

    #[test]
    fn reports_failed_repo_with_git_output_under_table_when_git_fails() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.registered_repo("web");
        sandbox.wrkt(&["new", "PROJ-123", "api", "web"]).success();
        sandbox.git(&api, &["update-ref", "-d", "refs/remotes/origin/main"]);

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        assert!(run.failure().line_for("api").contains("failed"), "{run}");
        assert!(run.line_for("web").contains("clean"), "{run}");
        let stdout = run.stdout();
        let table_end = stdout.find(&run.line_for("web")).unwrap();
        let git_output = stdout.find("unknown revision").expect("git output shown");
        assert!(git_output > table_end, "{run}");
    }

    #[test]
    fn streams_git_output_when_verbose() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        std::fs::write(worktree.join("UNTRACKED.md"), "new\n").unwrap();

        // Act
        let run = sandbox.wrkt(&["-v", "status", "PROJ-123"]);

        // Assert
        let stderr = run.success().stderr();
        assert!(stderr.contains("git status --porcelain"), "{run}");
        assert!(stderr.contains("UNTRACKED.md"), "{run}");
    }

    #[test]
    fn hides_git_output_when_not_verbose() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        std::fs::write(worktree.join("UNTRACKED.md"), "new\n").unwrap();

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-123"]);

        // Assert
        assert!(!run.success().stderr().contains("UNTRACKED.md"), "{run}");
        assert!(!run.stdout().contains("UNTRACKED.md"), "{run}");
    }

    #[test]
    fn infers_workspace_when_name_omitted_inside_it() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let inside = sandbox.workspace_root().join("PROJ-123/api");

        // Act
        let run = sandbox.wrkt_in(&inside, &["status"]);

        // Assert
        assert!(run.success().line_for("api").contains("PROJ-123"), "{run}");
    }

    #[test]
    fn rejects_omitted_name_when_outside_workspace_root() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["status"]);

        // Assert
        assert!(run.failure().stderr().contains("pass a Workspace name"));
    }

    #[test]
    fn rejects_workspace_when_missing() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["status", "PROJ-999"]);

        // Assert
        assert!(
            run.failure()
                .stderr()
                .contains("no Workspace named 'PROJ-999'")
        );
    }
}

mod sync {
    use super::support::Sandbox;

    #[test]
    fn rebases_repo_onto_origin_main_when_behind() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        let work = sandbox.commit_in(&worktree, "WORK.md");
        let colleague = sandbox.advance_origin_main("api");

        // Act
        let run = sandbox.wrkt(&["sync", "PROJ-123"]);

        // Assert
        assert!(run.success().line_for("api").contains("rebased"), "{run}");
        assert_eq!(sandbox.git(&worktree, &["rev-parse", "HEAD~1"]), colleague);
        assert_ne!(sandbox.git(&worktree, &["rev-parse", "HEAD"]), work);
    }

    #[test]
    fn reports_up_to_date_when_level_with_origin_main() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["sync", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("up to date"), "{line}");
        assert!(!line.contains("rebased"), "{line}");
    }

    #[test]
    fn leaves_rebase_in_progress_when_conflicting() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.registered_repo("web");
        sandbox.wrkt(&["new", "PROJ-123", "api", "web"]).success();
        let api = sandbox.workspace_root().join("PROJ-123/api");
        let web = sandbox.workspace_root().join("PROJ-123/web");
        sandbox.commit_in(&api, "CHANGE.md");
        sandbox.advance_origin_main("api");
        let colleague = sandbox.advance_origin_main("web");

        // Act
        let run = sandbox.wrkt(&["sync", "PROJ-123"]);

        // Assert
        assert!(
            run.failure().line_for("api").contains("conflicted"),
            "{run}"
        );
        assert!(run.line_for("web").contains("rebased"), "{run}");
        let rebase_head = sandbox.git(&api, &["rev-parse", "--verify", "REBASE_HEAD"]);
        assert!(!rebase_head.is_empty());
        assert_eq!(sandbox.git(&web, &["rev-parse", "HEAD"]), colleague);
    }

    #[test]
    fn skips_repo_without_stashing_when_dirty() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        let head = sandbox.git(&worktree, &["rev-parse", "HEAD"]);
        std::fs::write(worktree.join("README.md"), "edited\n").unwrap();
        sandbox.advance_origin_main("api");

        // Act
        let run = sandbox.wrkt(&["sync", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("skipped"), "{line}");
        assert!(line.contains("uncommitted changes"), "{line}");
        assert_eq!(sandbox.git(&worktree, &["rev-parse", "HEAD"]), head);
        assert_eq!(
            std::fs::read_to_string(worktree.join("README.md")).unwrap(),
            "edited\n"
        );
        assert_eq!(sandbox.git(&worktree, &["stash", "list"]), "");
    }

    #[test]
    fn skips_repo_when_rebase_in_progress() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        sandbox.rebase_into_conflict("api", &api, &worktree);
        let rebase_head = sandbox.git(&worktree, &["rev-parse", "REBASE_HEAD"]);
        let conflicted = std::fs::read_to_string(worktree.join("CHANGE.md")).unwrap();

        // Act
        let run = sandbox.wrkt(&["sync", "PROJ-123"]);

        // Assert
        let line = run.success().line_for("api");
        assert!(line.contains("skipped: rebase in progress"), "{line}");
        assert_eq!(
            sandbox.git(&worktree, &["rev-parse", "REBASE_HEAD"]),
            rebase_head
        );
        let after = std::fs::read_to_string(worktree.join("CHANGE.md")).unwrap();
        assert_eq!(after, conflicted);
    }

    #[test]
    fn reports_failed_repo_while_others_sync_when_fetch_fails() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.registered_repo("api");
        sandbox.registered_repo("web");
        sandbox.wrkt(&["new", "PROJ-123", "api", "web"]).success();
        sandbox.break_origin(&api);
        sandbox.advance_origin_main("web");

        // Act
        let run = sandbox.wrkt(&["sync", "PROJ-123"]);

        // Assert
        assert!(run.failure().line_for("api").contains("failed"), "{run}");
        assert!(run.line_for("web").contains("rebased"), "{run}");
        assert!(run.stdout().contains("/nonexistent/origin.git"), "{run}");
    }

    #[test]
    fn ignores_remote_branch_commits_when_local_branch_behind_remote() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        sandbox.commit_in(&worktree, "WORK.md");
        sandbox.commit_in(&worktree, "REMOTE_ONLY.md");
        sandbox.git(&worktree, &["push", "-u", "origin", "PROJ-123"]);
        sandbox.git(&worktree, &["reset", "--hard", "HEAD~1"]);
        sandbox.advance_origin_main("api");

        // Act
        let run = sandbox.wrkt(&["sync", "PROJ-123"]);

        // Assert
        assert!(run.success().line_for("api").contains("rebased"), "{run}");
        assert!(worktree.join("CHANGE.md").exists());
        assert!(worktree.join("WORK.md").exists());
        assert!(!worktree.join("REMOTE_ONLY.md").exists());
    }

    #[test]
    fn infers_workspace_when_run_inside_it() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");

        // Act
        let run = sandbox.wrkt_in(&worktree, &["sync"]);

        // Assert
        assert!(
            run.success().line_for("api").contains("up to date"),
            "{run}"
        );
    }

    #[test]
    fn fetches_skipped_repo_when_dirty() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        let worktree = sandbox.workspace_root().join("PROJ-123/api");
        std::fs::write(worktree.join("README.md"), "edited\n").unwrap();
        let colleague = sandbox.advance_origin_main("api");

        // Act
        let run = sandbox.wrkt(&["sync", "PROJ-123"]);

        // Assert
        assert!(run.success().line_for("api").contains("skipped"), "{run}");
        assert_eq!(
            sandbox.git(&worktree, &["rev-parse", "origin/main"]),
            colleague
        );
    }
}
