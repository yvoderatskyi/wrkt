mod support;

mod repo_add {
    use super::support::Sandbox;

    #[test]
    fn registers_repo_under_directory_name_when_no_name_given() {
        // Arrange
        let sb = Sandbox::new();
        let api = sb.repo("api");

        // Act
        sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

        // Assert
        let out = sb.wrkt(&["repo", "list"]).success().stdout();
        let line = out.lines().find(|l| l.contains("api")).expect("api listed");
        assert!(
            line.contains(api.to_str().unwrap()),
            "path missing from: {line}"
        );
    }

    #[test]
    fn registers_repo_under_flag_name_when_name_given() {
        // Arrange
        let sb = Sandbox::new();
        let api = sb.repo("api");

        // Act
        sb.wrkt(&["repo", "add", api.to_str().unwrap(), "--name", "backend"])
            .success();

        // Assert
        let out = sb.wrkt(&["repo", "list"]).success().stdout();
        let names: Vec<_> = out
            .lines()
            .filter_map(|l| l.split_whitespace().next())
            .collect();
        assert_eq!(names, ["backend"], "listed:\n{out}");
    }

    #[test]
    fn rejects_path_when_not_a_git_repository() {
        // Arrange
        let sb = Sandbox::new();
        let plain = sb.src.join("plain");
        std::fs::create_dir_all(&plain).unwrap();

        // Act
        let run = sb.wrkt(&["repo", "add", plain.to_str().unwrap()]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("not a git repository"),
            "stderr: {}",
            run.stderr()
        );
        assert_eq!(sb.wrkt(&["repo", "list"]).success().stdout(), "");
    }

    #[test]
    fn rejects_path_when_subdirectory_of_clone() {
        // Arrange
        let sb = Sandbox::new();
        let api = sb.repo("api");
        let sub = api.join("sub");
        std::fs::create_dir_all(&sub).unwrap();

        // Act
        let run = sb.wrkt(&["repo", "add", sub.to_str().unwrap()]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("top level"),
            "stderr: {}",
            run.stderr()
        );
        assert_eq!(sb.wrkt(&["repo", "list"]).success().stdout(), "");
    }

    #[test]
    fn rejects_path_when_linked_worktree() {
        // Arrange
        let sb = Sandbox::new();
        let api = sb.repo("api");
        let wt = sb.src.join("api-wt");
        sb.git(
            &api,
            &["worktree", "add", "-b", "side", wt.to_str().unwrap()],
        );

        // Act
        let run = sb.wrkt(&["repo", "add", wt.to_str().unwrap()]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("worktree"),
            "stderr: {}",
            run.stderr()
        );
        assert_eq!(sb.wrkt(&["repo", "list"]).success().stdout(), "");
    }

    #[test]
    fn rejects_name_when_unusable_as_directory() {
        // Arrange
        let sb = Sandbox::new();
        let api = sb.repo("api");

        // Act
        let runs: Vec<_> = ["", "a/b", ".", ".."]
            .into_iter()
            .map(|bad| {
                (
                    bad,
                    sb.wrkt(&["repo", "add", api.to_str().unwrap(), "--name", bad]),
                )
            })
            .collect();

        // Assert
        for (bad, run) in &runs {
            run.failure();
            assert!(
                run.stderr().contains("invalid Repo name"),
                "{bad:?}: {}",
                run.stderr()
            );
        }
        assert_eq!(sb.wrkt(&["repo", "list"]).success().stdout(), "");
    }

    #[test]
    fn rejects_name_when_already_registered() {
        // Arrange
        let sb = Sandbox::new();
        let api = sb.repo("api");
        let other = sb.repo("other");
        sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

        // Act
        let run = sb.wrkt(&["repo", "add", other.to_str().unwrap(), "--name", "api"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("already registered"),
            "stderr: {}",
            run.stderr()
        );
        let out = sb.wrkt(&["repo", "list"]).success().stdout();
        assert!(
            out.contains(api.to_str().unwrap()),
            "original entry lost:\n{out}"
        );
        assert!(
            !out.contains(other.to_str().unwrap()),
            "duplicate registered:\n{out}"
        );
    }

    #[test]
    fn rejects_clone_when_already_registered() {
        // Arrange
        let sb = Sandbox::new();
        let api = sb.repo("api");
        sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

        // Act
        let run = sb.wrkt(&["repo", "add", api.to_str().unwrap(), "--name", "again"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("already registered as 'api'"),
            "stderr: {}",
            run.stderr()
        );
        let out = sb.wrkt(&["repo", "list"]).success().stdout();
        assert!(!out.contains("again"), "registered twice:\n{out}");
    }
}

mod repo_rm {
    use super::support::Sandbox;

    #[test]
    fn unregisters_repo_when_registered() {
        // Arrange
        let sb = Sandbox::new();
        let api = sb.repo("api");
        let web = sb.repo("web");
        sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();
        sb.wrkt(&["repo", "add", web.to_str().unwrap()]).success();

        // Act
        sb.wrkt(&["repo", "rm", "api"]).success();

        // Assert
        let out = sb.wrkt(&["repo", "list"]).success().stdout();
        assert!(!out.contains("api"), "api still listed:\n{out}");
        assert!(out.contains("web"), "web lost:\n{out}");
        assert!(api.exists(), "repo rm must not touch the clone");
    }

    #[test]
    fn rejects_name_when_not_registered() {
        // Arrange
        let sb = Sandbox::new();

        // Act
        let run = sb.wrkt(&["repo", "rm", "nope"]);

        // Assert
        run.failure();
        assert!(run.stderr().contains("nope"), "stderr: {}", run.stderr());
    }
}

mod config {
    use super::support::Sandbox;

    #[test]
    fn creates_no_file_when_missing_and_only_read() {
        // Arrange
        let sb = Sandbox::new();

        // Act
        let run = sb.wrkt(&["repo", "list"]);

        // Assert
        assert_eq!(run.success().stdout(), "");
        assert!(
            !sb.config_path().exists(),
            "a read must not create the config"
        );
    }

    #[test]
    fn creates_file_with_defaults_when_first_written() {
        // Arrange
        let sb = Sandbox::new();
        let api = sb.repo("api");

        // Act
        sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

        // Assert
        let text = std::fs::read_to_string(sb.config_path()).expect("config written");
        let default_root = sb.home.join("wrkt");
        assert!(
            text.contains(default_root.to_str().unwrap()),
            "default root missing:\n{text}"
        );
        assert!(
            text.contains("{key}"),
            "default branch template missing:\n{text}"
        );
    }

    #[test]
    fn keeps_workspace_root_when_repo_added() {
        // Arrange
        let sb = Sandbox::new();
        sb.write_config("");
        let api = sb.repo("api");

        // Act
        sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

        // Assert
        let text = std::fs::read_to_string(sb.config_path()).unwrap();
        assert!(
            text.contains(sb.workspace_root.to_str().unwrap()),
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
        let sb = Sandbox::new();
        let xdg = sb.home.join("xdg");
        let api = sb.repo("api");

        // Act
        sb.wrkt_with(
            &sb.home,
            &[("XDG_CONFIG_HOME", xdg.to_str().unwrap())],
            &["repo", "add", api.to_str().unwrap()],
        )
        .success();

        // Assert
        assert!(xdg.join("wrkt/config.toml").exists());
        assert!(!sb.config_path().exists());
    }

    #[test]
    fn rejects_file_when_key_unknown() {
        // Arrange
        let sb = Sandbox::new();
        sb.write_config("workspace-root = \"/typo\"\n");

        // Act
        let run = sb.wrkt(&["repo", "list"]);

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
        let sb = Sandbox::new();
        sb.write_config("[repos]\n\"a/b\" = \"/somewhere\"\n");

        // Act
        let run = sb.wrkt(&["repo", "list"]);

        // Assert
        run.failure();
        assert!(
            run.stderr().contains("invalid Repo name"),
            "stderr: {}",
            run.stderr()
        );
    }
}
