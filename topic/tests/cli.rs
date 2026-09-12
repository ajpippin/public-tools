use assert_cmd::Command;
use predicates::prelude::*;

fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let topics = dir.path().join("topics");
    std::fs::create_dir(&topics).unwrap();
    std::fs::write(
        topics.join("0001.txt"),
        "@topic First topic\n@search alpha beta\nBody needle\n",
    )
    .unwrap();
    let config = dir.path().join("config.yaml");
    std::fs::write(&config, format!("topic_dir: {}\nauthor_name: Test User\nauthor_email: test@example.com\ngpg_binary: gpg\naliases:\n  q: search\n  p: print\n", topics.display())).unwrap();
    (dir, config)
}

#[test]
fn help_has_complete_commands() {
    Command::cargo_bin("topic")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("sections")
                .and(predicate::str::contains("edit"))
                .and(predicate::str::contains("search")),
        );
}
#[test]
fn configured_alias_searches() {
    let (_dir, config) = fixture();
    Command::cargo_bin("topic")
        .unwrap()
        .args([
            "--config",
            config.to_str().unwrap(),
            "q",
            "alpha",
            "--no-pager",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Body needle"));
}
#[test]
fn initializes_without_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("nested/config.yaml");
    let args = ["--config", config.to_str().unwrap(), "init"];
    Command::cargo_bin("topic")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created configuration"));
    Command::cargo_bin("topic")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}
