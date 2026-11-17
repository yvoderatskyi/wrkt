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
