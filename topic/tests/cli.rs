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

/// An isolated topic library: HOME, the config, a `topics` git checkout
/// with a bare `origin`, a fake `gpg`, and scripted editors all live in one
/// temp directory.
struct Library {
    dir: tempfile::TempDir,
}

impl Library {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let topics = root.join("topics");
        std::fs::create_dir(&topics).unwrap();
        for (name, text) in [
            (
                "0001.txt",
                "@topic First topic\n@search alpha beta\nBody needle\n",
            ),
            ("0100.md", "@topic ~~~ Tools section\n"),
            (
                "0101.md",
                "@topic Rust notes\n@search cargo alpha\nUse clippy.\n",
            ),
            ("0200.md", "@topic ~~~ Home section\n"),
        ] {
            std::fs::write(topics.join(name), text).unwrap();
        }
        std::fs::write(
            topics.join("0300.md.gpg"),
            "FAKEGPG\n@topic Secret\nhidden body\n",
        )
        .unwrap();
        std::fs::write(
            root.join("config.yaml"),
            format!(
                "topic_dir: {}\nauthor_name: Test User\nauthor_email: test@example.com\n\
                 gpg_binary: {}\ngpg_recipients: [test@example.com]\n\
                 aliases:\n  q: search\n  p: print\n  s: sections\n",
                topics.display(),
                root.join("bin/gpg").display()
            ),
        )
        .unwrap();
        let bin = root.join("bin");
        std::fs::create_dir(&bin).unwrap();
        // Fake gpg: "encrypted" data is the plaintext behind a FAKEGPG line.
        script(
            &bin.join("gpg"),
            &format!(
                "for last; do :; done\n\
                 echo \"$*\" >> '{log}'\n\
                 case \" $* \" in\n\
                 *' --decrypt '*)\n\
                   [ \"$(head -n 1 \"$last\")\" = FAKEGPG ] || {{ echo 'bad packet' >&2; exit 2; }}\n\
                   tail -n +2 \"$last\" ;;\n\
                 *' --encrypt '*) echo FAKEGPG; cat \"$last\" ;;\n\
                 esac\n",
                log = root.join("gpg.log").display()
            ),
        );
        script(&bin.join("append-editor"), "echo 'edited line' >> \"$1\"\n");
        let library = Self { dir };
        library.git(&["init", "-q", "--bare", "-b", "main", "../origin.git"]);
        library.git(&["init", "-q", "-b", "main"]);
        library.git(&["remote", "add", "origin", "../origin.git"]);
        library.git(&["add", "0001.txt", "0100.md", "0101.md", "0200.md"]);
        library.git(&["commit", "-q", "-m", "Initial topics"]);
        library.git(&["push", "-q", "-u", "origin", "main"]);
        library
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        self.dir.path().join(name)
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.path(name)).unwrap()
    }

    fn git(&self, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(self.path("topics"))
            .envs(self.env())
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    fn env(&self) -> Vec<(&'static str, std::ffi::OsString)> {
        let root = self.dir.path();
        vec![
            ("HOME", root.into()),
            ("GIT_CONFIG_NOSYSTEM", "1".into()),
            ("GIT_AUTHOR_NAME", "Test User".into()),
            ("GIT_AUTHOR_EMAIL", "test@example.com".into()),
            ("GIT_COMMITTER_NAME", "Test User".into()),
            ("GIT_COMMITTER_EMAIL", "test@example.com".into()),
        ]
    }

    /// `topic` with no `--config`, isolated from the real HOME.
    fn bare_command(&self) -> Command {
        let mut command = Command::cargo_bin("topic").unwrap();
        command
            .current_dir(self.dir.path())
            .envs(self.env())
            .env("VISUAL", self.path("bin/append-editor"))
            .env_remove("EDITOR")
            .env_remove("TOPIC_CONFIG")
            .env_remove("XDG_CONFIG_HOME");
        command
    }

    fn command(&self) -> Command {
        let mut command = self.bare_command();
        command.args(["--config", self.path("config.yaml").to_str().unwrap()]);
        command
    }
}

fn script(path: &std::path::Path, body: &str) {
    std::fs::write(path, format!("#!/bin/sh\n{body}")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn help_has_complete_commands() {
    let library = Library::new();
    library
        .bare_command()
        .arg("--help")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("sections")
                .and(predicate::str::contains("edit"))
                .and(predicate::str::contains("search")),
        );
    // Alias resolution creates the default config under the isolated HOME.
    assert!(library.path(".config/tools/topic.yaml").exists());
}

#[test]
fn version_flag_and_subcommand_agree() {
    let library = Library::new();
    let flag = library.command().arg("--version").output().unwrap();
    let subcommand = library.command().arg("version").output().unwrap();
    assert!(flag.status.success() && subcommand.status.success());
    assert_eq!(flag.stdout, subcommand.stdout);
    assert!(String::from_utf8_lossy(&flag.stdout).starts_with("topic v"));
}

#[test]
fn lists_topics_sections_and_one_section() {
    let library = Library::new();
    library.command().arg("list").assert().success().stdout(
        "0001: First topic\n0100: ~~~ Tools section\n0101: Rust notes\n\
             0200: ~~~ Home section\n0300: Secret\n",
    );
    library
        .command()
        .args(["list", "01"])
        .assert()
        .success()
        .stdout("0100: ~~~ Tools section\n0101: Rust notes\n");
    for sections in ["sections", "s"] {
        library
            .command()
            .arg(sections)
            .assert()
            .success()
            .stdout("0100: ~~~ Tools section\n0200: ~~~ Home section\n");
    }
}

#[test]
fn prints_several_topics() {
    let library = Library::new();
    library
        .command()
        .args(["--no-pager", "p", "1", "0101.md"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("-- 0001.txt ---")
                .and(predicate::str::contains("Body needle"))
                .and(predicate::str::contains("-- 0101.md ---"))
                .and(predicate::str::contains("Use clippy.")),
        );
    library
        .command()
        .args(["print", "42"])
        .assert()
        .failure()
        .stderr("42: no topic found\n");
    library.command().arg("print").assert().failure();
}

#[test]
fn search_lists_matches_or_shows_single_topic() {
    let library = Library::new();
    // Bare expressions search metadata; two matches are listed.
    library
        .command()
        .arg("ALPHA")
        .assert()
        .success()
        .stdout("0001: alpha beta\n0101: cargo alpha\n");
    // One match displays the topic itself.
    library
        .command()
        .args(["q", "cargo"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("-- 0101.md ---").and(predicate::str::contains("Use clippy.")),
        );
    // Metadata search misses body text; --all finds it.
    library
        .command()
        .args(["search", "clippy"])
        .assert()
        .success()
        .stdout("");
    library
        .command()
        .args(["search", "--all", "clippy", "needle"])
        .assert()
        .success()
        .stdout("0001.txt: Body needle\n0101.md: Use clippy.\n");
    library
        .command()
        .args(["search", "("])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid search expression"));
}

#[test]
fn dir_option_overrides_topic_dir_and_alias_still_resolves() {
    let library = Library::new();
    let other = library.path("other");
    std::fs::create_dir(&other).unwrap();
    std::fs::write(other.join("0007.md"), "@topic Elsewhere\n").unwrap();
    library
        .command()
        .args(["--dir", other.to_str().unwrap(), "--no-pager", "p", "7"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Elsewhere"));
}

#[test]
fn config_from_environment_and_default_creation() {
    let library = Library::new();
    library
        .bare_command()
        .env("TOPIC_CONFIG", library.path("config.yaml"))
        .arg("s")
        .assert()
        .success()
        .stdout(predicate::str::contains("Tools section"));
    library
        .bare_command()
        .args([
            "--dir",
            library.path("topics").to_str().unwrap(),
            "list",
            "02",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("created default configuration"))
        .stdout("0200: ~~~ Home section\n");
    assert!(library.path(".config/tools/topic.yaml").exists());
}

#[test]
fn config_equals_form_is_used_for_aliases() {
    let library = Library::new();
    let config = format!("--config={}", library.path("config.yaml").display());
    library
        .bare_command()
        .args([config.as_str(), "s"])
        .assert()
        .success()
        .stdout("0100: ~~~ Tools section\n0200: ~~~ Home section\n");
    assert!(!library.path(".config/tools/topic.yaml").exists());
}

#[test]
fn invalid_config_is_reported() {
    let library = Library::new();
    std::fs::write(library.path("config.yaml"), "aliases:\n  x: nope\n").unwrap();
    library
        .command()
        .arg("list")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown command \"nope\""));
}

#[test]
fn prints_encrypted_topics_through_gpg() {
    let library = Library::new();
    library
        .command()
        .args(["--no-pager", "p", "300"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("-- 0300.md.gpg ---")
                .and(predicate::str::contains("hidden body")),
        );
    std::fs::write(library.path("topics/0300.md.gpg"), "garbage\n").unwrap();
    library
        .command()
        .args(["p", "300"])
        .assert()
        .failure()
        .stderr("decrypt 0300.md.gpg: bad packet\n");
}

#[test]
fn edit_commits_and_pushes() {
    let library = Library::new();
    library
        .command()
        .args(["edit", "1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Pushing to Git..."));
    let text = library.read("topics/0001.txt");
    assert!(text.contains("Body needle\nedited line\n\n---\nLast updated: "));
    assert!(text.ends_with(", by \"Test User <test@example.com>\"\n"));
    assert_eq!(
        library.git(&["--git-dir", "../origin.git", "log", "-1", "--format=%s"]),
        "Update topic 0001\n"
    );
    assert_eq!(
        library.git(&["status", "--porcelain", "--", "0001.txt"]),
        ""
    );
}

#[test]
fn edit_without_changes_does_not_commit() {
    let library = Library::new();
    library.command().args(["edit", "101"]).assert().success();
    let head = library.git(&["rev-parse", "HEAD"]);
    // The footer is already current, so a no-op edit leaves nothing to commit.
    library
        .command()
        .env("VISUAL", "true")
        .args(["edit", "101"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No changes to commit."));
    assert_eq!(library.git(&["rev-parse", "HEAD"]), head);
}

#[test]
fn edit_stops_when_editor_fails() {
    let library = Library::new();
    let before = library.read("topics/0001.txt");
    library
        .command()
        .env("VISUAL", "false")
        .args(["edit", "1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("editor exited with"));
    assert_eq!(library.read("topics/0001.txt"), before);
}

#[test]
fn edit_requires_author() {
    let library = Library::new();
    let config = library
        .read("config.yaml")
        .replace("author_name: Test User", "author_name: ''");
    std::fs::write(library.path("config.yaml"), config).unwrap();
    library
        .command()
        .args(["edit", "1"])
        .assert()
        .failure()
        .stderr("author_name is required in the topic configuration\n");
}

#[test]
fn edit_encrypted_topic_reencrypts_without_git() {
    let library = Library::new();
    library
        .command()
        .args(["edit", "300"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Updated encrypted topic locally"));
    let text = library.read("topics/0300.md.gpg");
    assert!(
        text.starts_with("FAKEGPG\n@topic Secret\nhidden body\nedited line\n\n---\nLast updated: ")
    );
    assert!(
        library
            .read("gpg.log")
            .contains("--encrypt --recipient test@example.com")
    );
    assert_eq!(library.git(&["log", "--format=%s"]), "Initial topics\n");
}

#[test]
fn edit_encrypted_topic_requires_recipients() {
    let library = Library::new();
    let config = library
        .read("config.yaml")
        .replace("gpg_recipients: [test@example.com]", "gpg_recipients: []");
    std::fs::write(library.path("config.yaml"), config).unwrap();
    library
        .command()
        .args(["edit", "300"])
        .assert()
        .failure()
        .stderr("gpg_recipients is required to edit encrypted topics\n");
}
