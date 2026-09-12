use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn init_and_list_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/shelves.yaml");
    Command::cargo_bin("ollama-shelf")
        .unwrap()
        .args([
            "--config",
            path.to_str().unwrap(),
            "--hostname",
            "test-host",
            "init",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("created"));
    Command::cargo_bin("ollama-shelf")
        .unwrap()
        .args([
            "--config",
            path.to_str().unwrap(),
            "--hostname",
            "test-host",
            "list",
            "core",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("== CORE ==")
                .and(predicate::str::contains("qwen2.5-coder:7b")),
        );
}

#[test]
fn init_refuses_to_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shelves.yaml");
    let args = [
        "--config",
        path.to_str().unwrap(),
        "--hostname",
        "test-host",
        "init",
    ];
    Command::cargo_bin("ollama-shelf")
        .unwrap()
        .args(args)
        .assert()
        .success();
    Command::cargo_bin("ollama-shelf")
        .unwrap()
        .args(args)
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}
