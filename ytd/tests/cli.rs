use assert_cmd::Command;
use predicates::prelude::*;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

struct Fixture {
    dir: tempfile::TempDir,
    config: PathBuf,
}

// A fake yt-dlp that records its arguments and environment, and exits with
// FAKE_EXIT.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("yt-dlp");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\nenv > \"$0.env\"\nexit \"${FAKE_EXIT:-0}\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        format!(
            "yt_dlp_binary = \"{}\"\ndownload_dir = \"/downloads\"\noutput_template = \"%(id)s\"\n",
            binary.display()
        ),
    )
    .unwrap();
    Fixture { dir, config }
}

fn ytd(home: &Path) -> Command {
    let mut command = Command::cargo_bin("ytd").unwrap();
    command
        .env("HOME", home)
        .env_remove("YTD_CONFIG")
        .env_remove("XDG_CONFIG_HOME");
    command
}

impl Fixture {
    fn run(&self) -> Command {
        let mut command = ytd(self.dir.path());
        command.arg("--config").arg(&self.config);
        command
    }

    fn add_config(&self, text: &str) {
        let mut config = std::fs::read_to_string(&self.config).unwrap();
        config.push_str(text);
        std::fs::write(&self.config, config).unwrap();
    }

    fn recorded_env(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("yt-dlp.env")).unwrap()
    }

    fn recorded(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("yt-dlp.args")).unwrap()
    }
}

#[test]
fn downloads_with_configured_settings() {
    let fixture = fixture();
    fixture
        .run()
        .args(["get", "https://example.com/a"])
        .assert()
        .success();
    assert_eq!(
        fixture.recorded(),
        "--paths\n/downloads\n--output\n%(id)s\n--\nhttps://example.com/a\n"
    );
}

#[test]
fn passes_options_through() {
    let fixture = fixture();
    fixture
        .run()
        .args([
            "get",
            "--audio",
            "--dir",
            "/music",
            "u1",
            "--",
            "--no-mtime",
        ])
        .assert()
        .success();
    assert_eq!(
        fixture.recorded(),
        "--paths\n/music\n--output\n%(id)s\n--extract-audio\n--no-mtime\n--\nu1\n"
    );
}

#[test]
fn reports_yt_dlp_failure() {
    let fixture = fixture();
    fixture
        .run()
        .args(["get", "u1"])
        .env("FAKE_EXIT", "3")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("failed: exit status: 3"));
}

#[test]
fn reports_missing_binary() {
    let fixture = fixture();
    std::fs::write(&fixture.config, "yt_dlp_binary = \"/missing/yt-dlp\"\n").unwrap();
    fixture
        .run()
        .args(["get", "u1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("run /missing/yt-dlp"));
}

#[test]
fn missing_config_aborts_with_help() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join(".config/etc/ytd.toml");
    ytd(dir.path())
        .args(["get", "u1"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(format!(
            "no configuration file at {}",
            config.display()
        )))
        .stderr(predicate::str::contains(format!(
            "ytd init > \"{}\"",
            config.display()
        )))
        .stderr(predicate::str::contains("Usage: ytd"));
    assert!(!dir.path().join(".config").exists());
}

#[test]
fn honors_config_environment_variable() {
    let fixture = fixture();
    ytd(fixture.dir.path())
        .env("YTD_CONFIG", &fixture.config)
        .args(["get", "u1"])
        .assert()
        .success();
    assert!(fixture.recorded().ends_with("--\nu1\n"));
}

#[test]
fn requires_a_url() {
    fixture()
        .run()
        .arg("get")
        .assert()
        .failure()
        .stderr(predicate::str::contains("<URL>"));
}

#[test]
fn reports_version() {
    let dir = tempfile::tempdir().unwrap();
    ytd(dir.path())
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^ytd \S+ ([0-9a-f]{7}|unknown)\n$").unwrap());
}

#[test]
fn lists_playlist_entries() {
    let fixture = fixture();
    fixture
        .run()
        .args(["list", "https://example.com/playlist"])
        .assert()
        .success();
    assert_eq!(
        fixture.recorded(),
        "--flat-playlist\n--print\n%(title)s [%(id)s]\n--\nhttps://example.com/playlist\n"
    );
}

#[test]
fn accepts_config_after_command() {
    let fixture = fixture();
    ytd(fixture.dir.path())
        .args(["list", "u1", "--config"])
        .arg(&fixture.config)
        .assert()
        .success();
    assert!(fixture.recorded().ends_with("--\nu1\n"));
}

#[test]
fn requires_a_command() {
    fixture()
        .run()
        .arg("u1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn passes_env_defaults_to_yt_dlp() {
    let fixture = fixture();
    fixture.add_config("[env]\nYTD_TEST_DEFAULT = \"from file\"\n");
    fixture.run().args(["get", "u1"]).assert().success();
    assert!(
        fixture
            .recorded_env()
            .contains("YTD_TEST_DEFAULT=from file\n")
    );
}

#[test]
fn environment_overrides_env_defaults() {
    let fixture = fixture();
    fixture.add_config("[env]\nYTD_TEST_DEFAULT = \"from file\"\n");
    fixture
        .run()
        .env("YTD_TEST_DEFAULT", "from environment")
        .args(["get", "u1"])
        .assert()
        .success();
    assert!(
        fixture
            .recorded_env()
            .contains("YTD_TEST_DEFAULT=from environment\n")
    );
}

#[test]
fn honors_xdg_config_home() {
    let fixture = fixture();
    let xdg = fixture.dir.path().join("xdg");
    std::fs::create_dir_all(xdg.join("etc")).unwrap();
    std::fs::rename(&fixture.config, xdg.join("etc/ytd.toml")).unwrap();
    ytd(fixture.dir.path())
        .env("XDG_CONFIG_HOME", &xdg)
        .args(["get", "u1"])
        .assert()
        .success();
    assert!(fixture.recorded().ends_with("--\nu1\n"));
}

#[test]
fn reports_invalid_config() {
    let fixture = fixture();
    fixture.add_config(
        "[env
",
    );
    fixture
        .run()
        .args(["get", "u1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("parse config"));
}

#[test]
fn expands_configured_alias() {
    let fixture = fixture();
    fixture.add_config("[aliases]\nm = \"get --audio --dir /music\"\n");
    fixture.run().args(["m", "u1"]).assert().success();
    assert_eq!(
        fixture.recorded(),
        "--paths\n/music\n--output\n%(id)s\n--extract-audio\n--\nu1\n"
    );
}

#[test]
fn alias_can_pass_yt_dlp_options() {
    let fixture = fixture();
    fixture.add_config("[aliases]\nhd = \"get -- --format best\"\n");
    fixture.run().args(["hd", "u1"]).assert().success();
    assert!(fixture.recorded().ends_with("--format\nbest\n--\nu1\n"));
}

#[test]
fn reads_aliases_from_config_equals_option() {
    let fixture = fixture();
    fixture.add_config("[aliases]\ns = \"list\"\n");
    ytd(fixture.dir.path())
        .arg(format!("--config={}", fixture.config.display()))
        .args(["s", "u1"])
        .assert()
        .success();
    assert!(fixture.recorded().starts_with("--flat-playlist\n"));
}

#[test]
fn init_prints_default_config_without_creating_it() {
    let dir = tempfile::tempdir().unwrap();
    ytd(dir.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("# ytd configuration."))
        .stdout(predicate::str::contains("# yt_dlp_binary = \"yt-dlp\"\n"));
    assert!(!dir.path().join(".config").exists());
}

#[test]
fn init_output_is_a_usable_config() {
    let dir = tempfile::tempdir().unwrap();
    let output = ytd(dir.path()).arg("init").output().unwrap();
    let config = dir.path().join(".config/etc/ytd.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(config, output.stdout).unwrap();
    ytd(dir.path())
        .env("PATH", "")
        .args(["get", "u1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("run yt-dlp"));
}

#[test]
fn init_help_and_version_ignore_broken_config() {
    let fixture = fixture();
    fixture.add_config("[env\n");
    for arg in ["init", "--help", "--version"] {
        fixture.run().arg(arg).assert().success();
    }
    fixture.run().args(["get", "u1"]).assert().failure();
}

#[test]
fn help_works_without_config() {
    let dir = tempfile::tempdir().unwrap();
    ytd(dir.path())
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("init"));
}

#[test]
fn init_can_be_reached_through_an_alias() {
    let fixture = fixture();
    fixture.add_config("[aliases]\ni = \"init\"\n");
    fixture
        .run()
        .arg("i")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("# ytd configuration."));
}
