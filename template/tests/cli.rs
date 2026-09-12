use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn initializes_and_reuses_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/config.yaml");
    let args = ["--config", path.to_str().unwrap(), "init"];
    Command::cargo_bin("template")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created configuration"));
    Command::cargo_bin("template")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .stdout(predicate::str::contains("already exists"));
}

#[test]
fn creates_file_from_configured_template() {
    let dir = tempfile::tempdir().unwrap();
    let templates = dir.path().join("templates");
    std::fs::create_dir(&templates).unwrap();
    std::fs::write(templates.join("txt"), "name=%%fIlename%%\n").unwrap();
    let config = dir.path().join("config.yaml");
    std::fs::write(
        &config,
        format!(
            "template_dirs:\n  - {}\nauthor_name: Test\nauthor_email: test@example.com\n",
            templates.display()
        ),
    )
    .unwrap();
    let target = dir.path().join("example.txt");
    Command::cargo_bin("template")
        .unwrap()
        .args([
            "--config",
            config.to_str().unwrap(),
            target.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(target).unwrap(),
        "name=example.txt\n"
    );
}
