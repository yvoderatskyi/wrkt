mod support;

use support::Sandbox;

#[test]
fn repo_add_registers_under_directory_name_and_list_shows_it() {
    let sb = Sandbox::new();
    let api = sb.repo("api");

    sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

    let out = sb.wrkt(&["repo", "list"]).success().stdout();
    let line = out.lines().find(|l| l.contains("api")).expect("api listed");
    assert!(
        line.contains(api.to_str().unwrap()),
        "path missing from: {line}"
    );
}

#[test]
fn repo_add_name_flag_overrides_directory_name() {
    let sb = Sandbox::new();
    let api = sb.repo("api");

    sb.wrkt(&["repo", "add", api.to_str().unwrap(), "--name", "backend"])
        .success();

    let out = sb.wrkt(&["repo", "list"]).success().stdout();
    let names: Vec<_> = out
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(names, ["backend"], "listed:\n{out}");
}

#[test]
fn repo_add_rejects_a_path_that_is_not_a_git_repository() {
    let sb = Sandbox::new();
    let plain = sb.src.join("plain");
    std::fs::create_dir_all(&plain).unwrap();

    let run = sb.wrkt(&["repo", "add", plain.to_str().unwrap()]);

    run.failure();
    assert!(
        run.stderr().contains("not a git repository"),
        "stderr: {}",
        run.stderr()
    );
    assert_eq!(sb.wrkt(&["repo", "list"]).success().stdout(), "");
}

#[test]
fn repo_add_rejects_a_name_that_is_already_registered() {
    let sb = Sandbox::new();
    let api = sb.repo("api");
    let other = sb.repo("other");
    sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

    let run = sb.wrkt(&["repo", "add", other.to_str().unwrap(), "--name", "api"]);

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
fn repo_rm_unregisters_a_repo() {
    let sb = Sandbox::new();
    let api = sb.repo("api");
    let web = sb.repo("web");
    sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();
    sb.wrkt(&["repo", "add", web.to_str().unwrap()]).success();

    sb.wrkt(&["repo", "rm", "api"]).success();

    let out = sb.wrkt(&["repo", "list"]).success().stdout();
    assert!(!out.contains("api"), "api still listed:\n{out}");
    assert!(out.contains("web"), "web lost:\n{out}");
    assert!(api.exists(), "repo rm must not touch the clone");
}

#[test]
fn repo_rm_of_an_unknown_name_fails() {
    let sb = Sandbox::new();

    let run = sb.wrkt(&["repo", "rm", "nope"]);

    run.failure();
    assert!(run.stderr().contains("nope"), "stderr: {}", run.stderr());
}

#[test]
fn without_a_config_file_the_registry_is_empty_and_reading_creates_no_file() {
    let sb = Sandbox::new();

    let run = sb.wrkt(&["repo", "list"]);

    assert_eq!(run.success().stdout(), "");
    assert!(
        !sb.config_path().exists(),
        "a read must not create the config"
    );
}

#[test]
fn without_a_config_file_the_first_write_creates_it_with_defaults() {
    let sb = Sandbox::new();
    let api = sb.repo("api");

    sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

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
fn an_existing_config_keeps_its_workspace_root_when_repos_are_added() {
    let sb = Sandbox::new();
    sb.write_config("");
    let api = sb.repo("api");

    sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

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
fn config_lives_under_xdg_config_home_when_set() {
    let sb = Sandbox::new();
    let xdg = sb.home.join("xdg");
    let api = sb.repo("api");

    sb.wrkt_with(
        &sb.home,
        &[("XDG_CONFIG_HOME", xdg.to_str().unwrap())],
        &["repo", "add", api.to_str().unwrap()],
    )
    .success();

    assert!(xdg.join("wrkt/config.toml").exists());
    assert!(!sb.config_path().exists());
}

#[test]
fn repo_add_rejects_a_subdirectory_of_a_clone() {
    let sb = Sandbox::new();
    let api = sb.repo("api");
    let sub = api.join("sub");
    std::fs::create_dir_all(&sub).unwrap();

    let run = sb.wrkt(&["repo", "add", sub.to_str().unwrap()]);

    run.failure();
    assert!(
        run.stderr().contains("top level"),
        "stderr: {}",
        run.stderr()
    );
    assert_eq!(sb.wrkt(&["repo", "list"]).success().stdout(), "");
}

#[test]
fn repo_add_rejects_a_linked_worktree() {
    let sb = Sandbox::new();
    let api = sb.repo("api");
    let wt = sb.src.join("api-wt");
    sb.git(
        &api,
        &["worktree", "add", "-b", "side", wt.to_str().unwrap()],
    );

    let run = sb.wrkt(&["repo", "add", wt.to_str().unwrap()]);

    run.failure();
    assert!(
        run.stderr().contains("worktree"),
        "stderr: {}",
        run.stderr()
    );
    assert_eq!(sb.wrkt(&["repo", "list"]).success().stdout(), "");
}

#[test]
fn repo_add_rejects_names_that_cannot_be_a_directory_name() {
    let sb = Sandbox::new();
    let api = sb.repo("api");

    let runs: Vec<_> = ["", "a/b", ".", ".."]
        .into_iter()
        .map(|bad| {
            (
                bad,
                sb.wrkt(&["repo", "add", api.to_str().unwrap(), "--name", bad]),
            )
        })
        .collect();

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
fn repo_add_rejects_a_clone_that_is_already_registered() {
    let sb = Sandbox::new();
    let api = sb.repo("api");
    sb.wrkt(&["repo", "add", api.to_str().unwrap()]).success();

    let run = sb.wrkt(&["repo", "add", api.to_str().unwrap(), "--name", "again"]);

    run.failure();
    assert!(
        run.stderr().contains("already registered as 'api'"),
        "stderr: {}",
        run.stderr()
    );
    let out = sb.wrkt(&["repo", "list"]).success().stdout();
    assert!(!out.contains("again"), "registered twice:\n{out}");
}

#[test]
fn a_config_with_an_unknown_key_is_rejected_naming_the_key() {
    let sb = Sandbox::new();
    sb.write_config("workspace-root = \"/typo\"\n");

    let run = sb.wrkt(&["repo", "list"]);

    run.failure();
    assert!(
        run.stderr().contains("workspace-root"),
        "stderr: {}",
        run.stderr()
    );
}

#[test]
fn a_config_with_an_invalid_repo_name_is_rejected() {
    let sb = Sandbox::new();
    sb.write_config("[repos]\n\"a/b\" = \"/somewhere\"\n");

    let run = sb.wrkt(&["repo", "list"]);

    run.failure();
    assert!(
        run.stderr().contains("invalid Repo name"),
        "stderr: {}",
        run.stderr()
    );
}
