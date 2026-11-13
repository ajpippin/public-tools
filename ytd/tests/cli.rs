use assert_cmd::Command;
use predicates::prelude::*;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

struct Fixture {
    dir: tempfile::TempDir,
    config: PathBuf,
}

// A fake yt-dlp that appends its arguments to a record of every run, records its
// environment, writes FAKE_SRT as a subtitle file under its --paths directory
// when it is fetching captions, and exits with FAKE_EXIT. Given --print-to-file,
// it saves an empty video and lists it in that file.
const YT_DLP: &str = r#"#!/bin/sh
printf '%s\n' "$@" >> "$0.args"
env > "$0.env"
case " $* " in *' --skip-download '*) if [ -n "$FAKE_SRT" ]; then printf '%s' "$FAKE_SRT" > "$2/Title [id].en.srt"; fi;; esac
previous=
for arg; do
  if [ "$previous" = after_move:filepath ]; then
    mkdir -p "$2"
    : > "$2/Title [id].mp4"
    printf '%s\n' "$2/Title [id].mp4" >> "$arg"
  fi
  previous=$arg
done
exit "${FAKE_EXIT:-0}"
"#;

// A fake ffmpeg that records its arguments, writes an empty file at its last
// argument, and exits with FAKE_FFMPEG_EXIT.
const FFMPEG: &str = r#"#!/bin/sh
printf '%s\n' "$@" > "$0.args"
for arg; do last=$arg; done
: > "$last"
exit "${FAKE_FFMPEG_EXIT:-0}"
"#;

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("yt-dlp");
    let ffmpeg = dir.path().join("ffmpeg");
    for (path, script) in [(&binary, YT_DLP), (&ffmpeg, FFMPEG)] {
        std::fs::write(path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        format!(
            "yt_dlp_binary = \"{}\"\nffmpeg_binary = \"{}\"\ndownload_dir = \"/downloads\"\noutput_template = \"%(id)s\"\n",
            binary.display(),
            ffmpeg.display()
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
        "--paths\n/downloads\n--output\n%(id)s\n--no-playlist\n--merge-output-format\nmp4\n--remux-video\nmp4\n--\nhttps://example.com/a\n"
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
        "--paths\n/music\n--output\n%(id)s\n--no-playlist\n--extract-audio\n--audio-format\nmp3\n--no-mtime\n--\nu1\n"
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
    let config = dir.path().join(".config/tools/ytd.toml");
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
        "--flat-playlist\n--dump-single-json\n--\nhttps://example.com/playlist\n"
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
    std::fs::create_dir_all(xdg.join("tools")).unwrap();
    std::fs::rename(&fixture.config, xdg.join("tools/ytd.toml")).unwrap();
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
        "--paths\n/music\n--output\n%(id)s\n--no-playlist\n--extract-audio\n--audio-format\nmp3\n--\nu1\n"
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
    let config = dir.path().join(".config/tools/ytd.toml");
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

#[test]
fn accepts_video_id_starting_with_a_dash() {
    let fixture = fixture();
    fixture
        .run()
        .args(["get", "-jAq6qRM4nk"])
        .assert()
        .success();
    assert!(
        fixture
            .recorded()
            .ends_with("--\nhttps://www.youtube.com/watch?v=-jAq6qRM4nk\n")
    );
}

#[test]
fn accepts_dashed_video_id_after_an_alias() {
    let fixture = fixture();
    fixture.add_config("[aliases]\nhd = \"get -- --format best\"\n");
    fixture.run().args(["hd", "-jAq6qRM4nk"]).assert().success();
    assert!(
        fixture
            .recorded()
            .ends_with("--format\nbest\n--\nhttps://www.youtube.com/watch?v=-jAq6qRM4nk\n")
    );
}

#[test]
fn mistyped_option_is_still_an_error() {
    fixture()
        .run()
        .args(["get", "--audoi", "u1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
}

#[test]
fn list_extracts_playlist_from_video_url() {
    let fixture = fixture();
    fixture
        .run()
        .args([
            "list",
            "https://www.youtube.com/watch?v=ajHK8gJcS_U&list=PLBM6cXvJ9jZShrKjJPWs8DxzND8c12u-t&pp=0gcJCR0Ga94AFGB0",
        ])
        .assert()
        .success();
    assert!(fixture.recorded().ends_with(
        "--\nhttps://www.youtube.com/playlist?list=PLBM6cXvJ9jZShrKjJPWs8DxzND8c12u-t\n"
    ));
}

const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nHello there\n\n2\n00:00:02,000 --> 00:00:03,000\nHello there\nand welcome\n";

#[test]
fn transcript_saves_plain_text_and_prints_its_path() {
    let fixture = fixture();
    let dir = fixture.dir.path().join("out");
    let text = dir.join("Title [id].en.txt");
    fixture
        .run()
        .env("FAKE_SRT", SRT)
        .args(["get", "--transcript", "--dir"])
        .arg(&dir)
        .arg("https://example.com/a")
        .assert()
        .success()
        .stdout(format!("{}\n", text.display()));
    assert_eq!(
        std::fs::read_to_string(text).unwrap(),
        "Hello there and welcome\n"
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    assert!(fixture.recorded().contains("--skip-download\n"));
}

#[test]
fn transcript_reports_when_there_are_no_captions() {
    let fixture = fixture();
    let dir = fixture.dir.path().join("out");
    fixture
        .run()
        .args(["get", "--transcript", "--dir"])
        .arg(&dir)
        .arg("u1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no transcript available"));
    assert!(!dir.exists());
}

#[test]
fn transcript_reports_yt_dlp_failure() {
    let fixture = fixture();
    fixture
        .run()
        .env("FAKE_EXIT", "3")
        .args(["get", "--transcript", "u1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed: exit status: 3"));
}

#[test]
fn transcript_can_be_reached_through_an_alias() {
    let fixture = fixture();
    fixture.add_config("[aliases]\ntranscript = \"get --transcript\"\n");
    let dir = fixture.dir.path().join("out");
    fixture
        .run()
        .env("FAKE_SRT", SRT)
        .args(["transcript", "--lang", "en", "--dir"])
        .arg(&dir)
        .arg("u1")
        .assert()
        .success();
    assert!(dir.join("Title [id].en.txt").exists());
}

#[test]
fn type_all_extracts_the_audio_from_the_video() {
    let fixture = fixture();
    let dir = fixture.dir.path().join("out");
    let video = dir.join("Title [id].mp4");
    let audio = dir.join("Title [id].mp3");
    let text = dir.join("Title [id].en.txt");
    fixture
        .run()
        .env("FAKE_SRT", SRT)
        .args(["get", "--type=all", "--dir"])
        .arg(&dir)
        .arg("u1")
        .assert()
        .success()
        .stdout(format!("{}\n{}\n", audio.display(), text.display()));
    assert!(video.exists() && audio.exists() && text.exists());
    let recorded = fixture.recorded();
    let runs: Vec<_> = recorded.split_inclusive("--\nu1\n").collect();
    assert_eq!(runs.len(), 2);
    assert!(runs[0].contains("--remux-video\nmp4\n--print-to-file\nafter_move:filepath\n"));
    assert!(!recorded.contains("--extract-audio"));
    assert!(runs[1].contains("--skip-download\n"));
    assert_eq!(
        std::fs::read_to_string(fixture.dir.path().join("ffmpeg.args")).unwrap(),
        format!(
            "-nostdin\n-loglevel\nerror\n-y\n-i\n{}\n-vn\n-codec:a\nlibmp3lame\n-q:a\n5\n{}\n",
            video.display(),
            audio.display()
        )
    );
}

#[test]
fn audio_alone_is_still_downloaded_by_yt_dlp() {
    let fixture = fixture();
    fixture
        .run()
        .args(["get", "--type", "mp3", "u1"])
        .assert()
        .success();
    assert!(fixture.recorded().contains("--extract-audio\n"));
    assert!(!fixture.dir.path().join("ffmpeg.args").exists());
}

#[test]
fn reports_ffmpeg_failure() {
    let fixture = fixture();
    fixture
        .run()
        .env("FAKE_FFMPEG_EXIT", "4")
        .args(["get", "--type", "mp4,mp3", "--dir"])
        .arg(fixture.dir.path().join("out"))
        .arg("u1")
        .assert()
        .failure()
        .stderr(predicate::str::contains("ffmpeg failed: exit status: 4"));
}

#[test]
fn type_stops_at_the_first_failed_run() {
    let fixture = fixture();
    fixture
        .run()
        .env("FAKE_EXIT", "3")
        .args(["get", "--type", "mp4,txt", "u1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("failed: exit status: 3"));
    assert_eq!(fixture.recorded().matches("--\nu1\n").count(), 1);
}

#[test]
fn lang_needs_a_transcript() {
    fixture()
        .run()
        .args(["get", "--lang", "fr", "u1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--lang needs --type txt"));
}
