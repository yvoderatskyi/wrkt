mod support;

mod repo_add {
    use super::support::Sandbox;

    #[test]
    fn registers_repo_under_directory_name_when_no_name_given() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");

        // Act
        sandbox
            .wrkt(&["repo", "add", api.to_str().unwrap()])
            .success();

        // Assert
        let listing = sandbox.wrkt(&["repo", "list"]).success().stdout();
        let line = listing
            .lines()
            .find(|l| l.contains("api"))
            .expect("api listed");
        assert!(
            line.contains(api.to_str().unwrap()),
            "path missing from: {line}"
        );
    }

    #[test]
    fn registers_repo_under_flag_name_when_name_given() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");

        // Act
        sandbox
            .wrkt(&["repo", "add", api.to_str().unwrap(), "--name", "backend"])
            .success();

        // Assert
        let listing = sandbox.wrkt(&["repo", "list"]).success().stdout();
        let names: Vec<_> = listing
            .lines()
            .filter_map(|l| l.split_whitespace().next())
            .collect();
        assert_eq!(names, ["backend"], "listed:\n{listing}");
    }

    #[test]
    fn rejects_path_when_not_a_git_repository() {
        // Arrange
        let sandbox = Sandbox::new();
        let plain = sandbox.clones().join("plain");
        std::fs::create_dir_all(&plain).unwrap();

        // Act
        let run = sandbox.wrkt(&["repo", "add", plain.to_str().unwrap()]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("not a git repository"),
            "stderr: {}",
            run.stderr()
        );
        assert_eq!(sandbox.wrkt(&["repo", "list"]).success().stdout(), "");
    }

    #[test]
    fn rejects_path_when_subdirectory_of_clone() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");
        let subdirectory = api.join("subdirectory");
        std::fs::create_dir_all(&subdirectory).unwrap();

        // Act
        let run = sandbox.wrkt(&["repo", "add", subdirectory.to_str().unwrap()]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("top level"),
            "stderr: {}",
            run.stderr()
        );
        assert_eq!(sandbox.wrkt(&["repo", "list"]).success().stdout(), "");
    }

    #[test]
    fn rejects_path_when_linked_worktree() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");
        let worktree = sandbox.clones().join("api-worktree");
        sandbox.git(
            &api,
            &["worktree", "add", "-b", "side", worktree.to_str().unwrap()],
        );

        // Act
        let run = sandbox.wrkt(&["repo", "add", worktree.to_str().unwrap()]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("worktree"),
            "stderr: {}",
            run.stderr()
        );
        assert_eq!(sandbox.wrkt(&["repo", "list"]).success().stdout(), "");
    }

    #[test]
    fn rejects_name_when_unusable_as_directory() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");

        // Act
        let attempts: Vec<_> = ["", "a/b", ".", ".."]
            .into_iter()
            .map(|name| {
                (
                    name,
                    sandbox.wrkt(&["repo", "add", api.to_str().unwrap(), "--name", name]),
                )
            })
            .collect();

        // Assert
        let accepted: Vec<_> = attempts
            .iter()
            .filter(|(_, run)| run.succeeded() || !run.stderr().contains("invalid Repo name"))
            .map(|(name, _)| *name)
            .collect();
        assert!(accepted.is_empty(), "not rejected as invalid: {accepted:?}");
        assert_eq!(sandbox.wrkt(&["repo", "list"]).success().stdout(), "");
    }

    #[test]
    fn rejects_name_when_already_registered() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");
        let other = sandbox.repo("other");
        sandbox
            .wrkt(&["repo", "add", api.to_str().unwrap()])
            .success();

        // Act
        let run = sandbox.wrkt(&["repo", "add", other.to_str().unwrap(), "--name", "api"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("already registered"),
            "stderr: {}",
            run.stderr()
        );
        let listing = sandbox.wrkt(&["repo", "list"]).success().stdout();
        assert!(
            listing.contains(api.to_str().unwrap()),
            "original entry lost:\n{listing}"
        );
        assert!(
            !listing.contains(other.to_str().unwrap()),
            "duplicate registered:\n{listing}"
        );
    }

    #[test]
    fn rejects_clone_when_already_registered() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");
        sandbox
            .wrkt(&["repo", "add", api.to_str().unwrap()])
            .success();

        // Act
        let run = sandbox.wrkt(&["repo", "add", api.to_str().unwrap(), "--name", "again"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("already registered as 'api'"),
            "stderr: {}",
            run.stderr()
        );
        let listing = sandbox.wrkt(&["repo", "list"]).success().stdout();
        assert!(!listing.contains("again"), "registered twice:\n{listing}");
    }

    #[test]
    fn aligns_clone_column_when_name_not_ascii() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        let web = sandbox.repo("web");
        sandbox
            .wrkt(&["repo", "add", web.to_str().unwrap(), "--name", "café"])
            .success();

        // Act
        let run = sandbox.wrkt(&["repo", "list"]);

        // Assert
        assert_eq!(run.success().second_column_offsets(), [6, 6], "{run}");
    }
}

mod repo_rm {
    use super::support::Sandbox;

    #[test]
    fn unregisters_repo_when_registered() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");
        let web = sandbox.repo("web");
        sandbox
            .wrkt(&["repo", "add", api.to_str().unwrap()])
            .success();
        sandbox
            .wrkt(&["repo", "add", web.to_str().unwrap()])
            .success();

        // Act
        sandbox.wrkt(&["repo", "rm", "api"]).success();

        // Assert
        let listing = sandbox.wrkt(&["repo", "list"]).success().stdout();
        assert!(!listing.contains("api"), "api still listed:\n{listing}");
        assert!(listing.contains("web"), "web lost:\n{listing}");
        assert!(api.exists(), "repo rm must not touch the clone");
    }

    #[test]
    fn rejects_name_when_not_registered() {
        // Arrange
        let sandbox = Sandbox::new();

        // Act
        let run = sandbox.wrkt(&["repo", "rm", "nope"]);

        // Assert
        run.failure();
        assert!(run.stderr().contains("nope"), "stderr: {}", run.stderr());
    }

    #[test]
    fn rejects_omitted_name_when_not_a_terminal() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");

        // Act
        let run = sandbox.wrkt(&["repo", "rm"]);

        // Assert
        run.failure();
        assert_eq!(run.stderr(), "error: no Repo given\n");
    }

    #[test]
    fn rejects_repo_when_a_workspace_uses_it() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Act
        let run = sandbox.wrkt(&["repo", "rm", "api"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("PROJ-123"),
            "stderr: {}",
            run.stderr()
        );
        let listing = sandbox.wrkt(&["repo", "list"]).success().stdout();
        assert!(listing.contains("api"), "api unregistered:\n{listing}");
    }

    #[test]
    fn unregisters_repo_when_its_workspace_was_removed() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.registered_repo("api");
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();
        sandbox.wrkt(&["rm", "PROJ-123"]).success();

        // Act
        let run = sandbox.wrkt(&["repo", "rm", "api"]);

        // Assert
        run.success();
    }
}

mod config {
    use std::path::Path;

    use super::support::Sandbox;

    #[test]
    fn creates_no_file_when_missing_and_only_read() {
        // Arrange
        let sandbox = Sandbox::new();

        // Act
        let run = sandbox.wrkt(&["repo", "list"]);

        // Assert
        assert_eq!(run.success().stdout(), "");
        assert!(
            !sandbox.config_path().exists(),
            "a read must not create the config"
        );
    }

    #[test]
    fn creates_file_without_default_root_when_first_written() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");

        // Act
        sandbox
            .wrkt(&["repo", "add", api.to_str().unwrap()])
            .success();

        // Assert
        let text = std::fs::read_to_string(sandbox.config_path()).expect("config written");
        assert!(!text.contains("root"), "default root written:\n{text}");
        assert!(
            text.contains("{key}"),
            "default branch template missing:\n{text}"
        );
    }

    #[test]
    fn creates_workspaces_under_home_when_no_root_configured() {
        // Arrange
        let sandbox = Sandbox::new();
        let api = sandbox.repo("api");
        sandbox
            .wrkt(&["repo", "add", api.to_str().unwrap()])
            .success();

        // Act
        sandbox.wrkt(&["new", "PROJ-123", "api"]).success();

        // Assert
        assert!(sandbox.home().join("wrkt/PROJ-123/api").is_dir());
    }

    #[test]
    fn creates_workspaces_under_home_when_root_starts_with_tilde() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config_rooted_at(Path::new("~/work"), "");
        sandbox.registered_repo("api");

        // Act
        let run = sandbox.wrkt(&["new", "PROJ-123", "api"]);

        // Assert
        run.success();
        let worktree = sandbox.home().join("work/PROJ-123/api");
        assert_eq!(
            sandbox.git(&worktree, &["branch", "--show-current"]),
            "PROJ-123"
        );
    }

    #[test]
    fn rejects_root_when_relative() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config_rooted_at(Path::new("work"), "");

        // Act
        let run = sandbox.wrkt(&["list"]);

        // Assert
        assert!(
            run.failure()
                .stderr()
                .contains("Workspace root 'work' must be an absolute path or start with ~/"),
            "{run}"
        );
    }

    #[test]
    fn rejects_root_when_relative_and_workspace_named() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config_rooted_at(Path::new("work"), "");

        // Act
        let run = sandbox.wrkt(&["which", "PROJ-123"]);

        // Assert
        assert!(
            run.failure()
                .stderr()
                .contains("Workspace root 'work' must be an absolute path or start with ~/"),
            "{run}"
        );
    }

    #[test]
    fn rejects_root_when_tilde_names_another_user() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config_rooted_at(Path::new("~bob/work"), "");

        // Act
        let run = sandbox.wrkt(&["list"]);

        // Assert
        assert!(
            run.failure()
                .stderr()
                .contains("Workspace root '~bob/work' must be an absolute path or start with ~/"),
            "{run}"
        );
    }

    #[test]
    fn keeps_workspace_root_when_repo_added() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config("");
        let api = sandbox.repo("api");

        // Act
        sandbox
            .wrkt(&["repo", "add", api.to_str().unwrap()])
            .success();

        // Assert
        let text = std::fs::read_to_string(sandbox.config_path()).unwrap();
        assert!(
            text.contains(sandbox.workspace_root().to_str().unwrap()),
            "root lost:\n{text}"
        );
        assert!(
            text.contains(api.to_str().unwrap()),
            "repo missing:\n{text}"
        );
    }

    #[test]
    fn uses_xdg_config_home_when_set() {
        // Arrange
        let sandbox = Sandbox::new();
        let xdg_config_home = sandbox.home().join("xdg_config_home");
        let api = sandbox.repo("api");

        // Act
        sandbox
            .wrkt_with(
                &sandbox.home(),
                &[("XDG_CONFIG_HOME", xdg_config_home.to_str().unwrap())],
                &["repo", "add", api.to_str().unwrap()],
            )
            .success();

        // Assert
        assert!(xdg_config_home.join("wrkt/config.toml").exists());
        assert!(!sandbox.config_path().exists());
    }

    #[test]
    fn rejects_default_root_when_home_unset() {
        // Arrange
        let sandbox = Sandbox::new();
        let xdg_config_home = sandbox.home().join("xdg_config_home");

        // Act
        let run = sandbox.wrkt_with(
            &sandbox.home(),
            &[
                ("XDG_CONFIG_HOME", xdg_config_home.to_str().unwrap()),
                ("HOME", ""),
            ],
            &["list"],
        );

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("HOME is not set"),
            "stderr: {}",
            run.stderr()
        );
    }

    #[test]
    fn accepts_named_workspace_when_home_unset() {
        // Arrange
        let sandbox = Sandbox::new();
        let xdg_config_home = sandbox.home().join("xdg_config_home");

        // Act
        let run = sandbox.wrkt_with(
            &sandbox.home(),
            &[
                ("XDG_CONFIG_HOME", xdg_config_home.to_str().unwrap()),
                ("HOME", ""),
            ],
            &["which", "PROJ-123"],
        );

        // Assert
        assert_eq!(run.success().stdout(), "PROJ-123\n");
    }

    #[test]
    fn rejects_file_when_key_unknown() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config("workspace-root = \"/typo\"\n");

        // Act
        let run = sandbox.wrkt(&["repo", "list"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("workspace-root"),
            "stderr: {}",
            run.stderr()
        );
    }

    #[test]
    fn rejects_file_when_repo_name_invalid() {
        // Arrange
        let sandbox = Sandbox::new();
        sandbox.write_config("[repos]\n\"a/b\" = \"/somewhere\"\n");

        // Act
        let run = sandbox.wrkt(&["repo", "list"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("invalid Repo name"),
            "stderr: {}",
            run.stderr()
        );
    }
}
