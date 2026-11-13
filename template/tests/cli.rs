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

/// A temp directory holding a `templates` directory and a config pointing
/// at it. Commands run inside the directory with HOME pointed at it, so the
/// user's own templates and git identity cannot leak in.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("templates");
        std::fs::create_dir(&templates).unwrap();
        std::fs::write(templates.join("txt"), "name=%%fIlename%% by %%author%%\n").unwrap();
        std::fs::write(templates.join("md"), "# %%Basename%%\n").unwrap();
        std::fs::write(templates.join("c"), "c:%%filename%%\n").unwrap();
        std::fs::write(templates.join("h"), "h:%%filename%%\n").unwrap();
        std::fs::write(
            dir.path().join("config.yaml"),
            format!(
                "template_dirs:\n  - {}\nauthor_name: Test\nauthor_email: test@example.com\n\
                 substitutions:\n  project: Tools\n",
                templates.display()
            ),
        )
        .unwrap();
        Self { dir }
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        self.dir.path().join(name)
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.path(name)).unwrap()
    }

    fn bare_command(&self) -> Command {
        let mut command = Command::cargo_bin("template").unwrap();
        command
            .current_dir(self.dir.path())
            .env("HOME", self.dir.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("TEMPLATE_CONFIG")
            .env_remove("TEMPLATE_DIR")
            .env_remove("XDG_CONFIG_HOME");
        command
    }

    fn command(&self) -> Command {
        let mut command = self.bare_command();
        command.args(["--config", self.path("config.yaml").to_str().unwrap()]);
        command
    }
}

#[test]
fn version_flag_and_subcommand_agree() {
    let fixture = Fixture::new();
    let flag = fixture.bare_command().arg("--version").output().unwrap();
    let subcommand = fixture.bare_command().arg("version").output().unwrap();
    assert!(flag.status.success() && subcommand.status.success());
    assert_eq!(flag.stdout, subcommand.stdout);
    assert!(String::from_utf8_lossy(&flag.stdout).starts_with("template v"));
}

#[test]
fn requires_a_target() {
    Fixture::new()
        .command()
        .assert()
        .failure()
        .stderr("at least one target is required\n");
}

#[test]
fn force_and_backup_conflict() {
    Fixture::new()
        .command()
        .args(["--force", "--backup", "a.txt"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn lists_templates_sorted() {
    let fixture = Fixture::new();
    let output = fixture.command().arg("list").output().unwrap();
    assert!(output.status.success());
    let names: Vec<_> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().next().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["c", "h", "md", "txt"]);
}

#[test]
fn show_prints_all_or_one_substitution() {
    let fixture = Fixture::new();
    fixture
        .command()
        .args(["show", "--target", "Report.md", "-t", "md"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("%%fIlename%%")
                .and(predicate::str::contains("-> Report.md"))
                .and(predicate::str::contains("-> Tools"))
                .and(predicate::str::contains("-> md\n")),
        );
    for query in ["project", "%%PROJECT%%"] {
        fixture
            .command()
            .args(["show", query])
            .assert()
            .success()
            .stdout(format!("{:<22} -> Tools\n", "%%project%%"));
    }
    fixture
        .command()
        .args(["show", "nope"])
        .assert()
        .failure()
        .stderr("substitution \"nope\" is not defined\n");
    fixture
        .command()
        .args(["show", "-t", "missing"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "template \"missing\" was not found",
        ));
}

#[test]
fn creates_several_targets_including_compound_ones() {
    let fixture = Fixture::new();
    fixture
        .command()
        .args(["notes.txt", "sub/readme.md", "pair.ch"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Created").count(4));
    assert_eq!(fixture.read("notes.txt"), "name=notes.txt by Test\n");
    assert_eq!(fixture.read("sub/readme.md"), "# Readme\n");
    assert_eq!(fixture.read("pair.c"), "c:pair.c\n");
    assert_eq!(fixture.read("pair.h"), "h:pair.h\n");
}

#[test]
fn explicit_template_is_used() {
    let fixture = Fixture::new();
    fixture
        .command()
        .args(["-t", "md", "notes.txt"])
        .assert()
        .success();
    assert_eq!(fixture.read("notes.txt"), "# Notes\n");
}

#[test]
fn missing_template_fails() {
    let fixture = Fixture::new();
    fixture
        .command()
        .arg("x.zzz")
        .assert()
        .failure()
        .stderr(predicate::str::contains("template \"zzz\" was not found"));
    assert!(!fixture.path("x.zzz").exists());
}

#[test]
fn existing_target_handling() {
    let fixture = Fixture::new();
    let target = fixture.path("a.txt");
    std::fs::write(&target, "old").unwrap();

    fixture
        .command()
        .arg("a.txt")
        .write_stdin("\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("[A/o/b]?"));
    assert_eq!(fixture.read("a.txt"), "old");

    fixture
        .command()
        .args(["--backup", "a.txt"])
        .assert()
        .success();
    assert_eq!(fixture.read("a.txt~"), "old");
    assert_eq!(fixture.read("a.txt"), "name=a.txt by Test\n");

    std::fs::write(&target, "edited").unwrap();
    fixture.command().args(["-f", "a.txt"]).assert().success();
    assert_eq!(fixture.read("a.txt"), "name=a.txt by Test\n");
    assert_eq!(fixture.read("a.txt~"), "old");

    std::fs::write(&target, "edited").unwrap();
    fixture
        .command()
        .arg("a.txt")
        .write_stdin("o\n")
        .assert()
        .success();
    assert_eq!(fixture.read("a.txt"), "name=a.txt by Test\n");
}

#[test]
fn config_comes_from_environment() {
    let fixture = Fixture::new();
    fixture
        .bare_command()
        .env("TEMPLATE_CONFIG", fixture.path("config.yaml"))
        .arg("env.md")
        .assert()
        .success();
    assert_eq!(fixture.read("env.md"), "# Env\n");
}

#[test]
fn default_config_is_created_and_template_dir_is_searched() {
    let fixture = Fixture::new();
    fixture
        .bare_command()
        .env("TEMPLATE_DIR", fixture.path("templates"))
        .arg("fresh.md")
        .assert()
        .success()
        .stderr(predicate::str::contains("created default configuration"));
    assert!(fixture.path(".config/tools/template.yaml").exists());
    assert_eq!(fixture.read("fresh.md"), "# Fresh\n");
}

#[test]
fn current_directory_templates_are_found() {
    let fixture = Fixture::new();
    std::fs::write(fixture.path("rs"), "local %%filename%%\n").unwrap();
    fixture
        .bare_command()
        .args(["--config", "missing.yaml", "lib.rs"])
        .assert()
        .success();
    assert_eq!(fixture.read("lib.rs"), "local lib.rs\n");
}
