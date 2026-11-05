use assert_cmd::Command;
use predicates::prelude::*;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

struct Fixture {
    dir: tempfile::TempDir,
    config: PathBuf,
}

// A fake yt-dlp that records its arguments and exits with FAKE_EXIT.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("yt-dlp");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\nexit \"${FAKE_EXIT:-0}\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = dir.path().join("config.yaml");
    std::fs::write(
        &config,
        format!(
            "yt_dlp_binary: {}\ndownload_dir: /downloads\noutput_template: '%(id)s'\n",
            binary.display()
        ),
    )
    .unwrap();
    Fixture { dir, config }
}

fn ytd(home: &Path) -> Command {
    let mut command = Command::cargo_bin("ytd").unwrap();
    command.env("HOME", home).env_remove("YTD_CONFIG");
    command
}

impl Fixture {
    fn run(&self) -> Command {
        let mut command = ytd(self.dir.path());
        command.arg("--config").arg(&self.config);
        command
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
    std::fs::write(&fixture.config, "yt_dlp_binary: /missing/yt-dlp\n").unwrap();
    fixture
        .run()
        .args(["get", "u1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("run /missing/yt-dlp"));
}

#[test]
fn creates_default_config_under_home() {
    let dir = tempfile::tempdir().unwrap();
    ytd(dir.path())
        .env("PATH", "")
        .args(["get", "u1"])
        .assert()
        .failure();
    let config = dir.path().join(".config/tools/ytd.yaml");
    assert!(
        std::fs::read_to_string(config)
            .unwrap()
            .contains("# yt_dlp_binary: yt-dlp")
    );
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
    let dir = tempfile::tempdir().unwrap();
    ytd(dir.path())
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
    let dir = tempfile::tempdir().unwrap();
    ytd(dir.path())
        .arg("u1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}
