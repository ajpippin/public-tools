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

const SHELF: &str = "hosts:
  - hostname: default
    shelves:
      core:
        - {name: 'tiny:1b', slot: tiny, purpose: routing}
        - {name: 'coder:7b', slot: coder, purpose: code}
      extra:
        - {name: 'thinker:8b', slot: reasoner, purpose: puzzles}
";

/// A temp directory holding a shelf config and a fake `ollama` executable
/// that logs its arguments and prints a canned `ollama list` table.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new(installed: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("shelves.yaml"), SHELF).unwrap();
        let mut list = String::from("NAME ID SIZE MODIFIED\n");
        for name in installed {
            list.push_str(&format!("{name} abc123 1.0 GB now\n"));
        }
        std::fs::write(dir.path().join("list.txt"), list).unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let script = bin.join("ollama");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n\
                 echo \"$*\" >> '{root}/calls.log'\n\
                 if [ -e '{root}/fail' ]; then echo boom >&2; exit 3; fi\n\
                 if [ \"$1\" = list ]; then cat '{root}/list.txt'; fi\n",
                root = dir.path().display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self { dir }
    }

    fn command(&self) -> Command {
        let mut command = Command::cargo_bin("ollama-shelf").unwrap();
        let path = format!(
            "{}:{}",
            self.dir.path().join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        command
            .env("PATH", path)
            .env("OLLAMA_MODELS", self.dir.path().join("models"))
            .env_remove("OLLAMA_HOST_URL")
            .env_remove("OLLAMA_SHELF_CONFIG")
            .env_remove("XDG_CONFIG_HOME")
            .args([
                "--config",
                self.dir.path().join("shelves.yaml").to_str().unwrap(),
                "--hostname",
                "devbox",
            ]);
        command
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("calls.log")).unwrap_or_default()
    }

    fn fail_ollama(&self) {
        std::fs::write(self.dir.path().join("fail"), "").unwrap();
    }
}

#[test]
fn list_all_prints_every_shelf() {
    let fixture = Fixture::new(&[]);
    fixture.command().arg("list").assert().success().stdout(
        predicate::str::contains("== CORE ==")
            .and(predicate::str::contains("== EXTRA =="))
            .and(predicate::str::contains("thinker:8b")),
    );
}

#[test]
fn list_unknown_shelf_fails() {
    Fixture::new(&[])
        .command()
        .args(["list", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("shelf \"nope\" is not configured"));
}

#[test]
fn missing_config_is_created_on_first_use() {
    let fixture = Fixture::new(&[]);
    let path = fixture.dir.path().join("shelves.yaml");
    std::fs::remove_file(&path).unwrap();
    fixture
        .command()
        .args(["list", "optional"])
        .assert()
        .success()
        .stderr(predicate::str::contains("created default configuration"))
        .stdout(predicate::str::contains("deepseek-r1:8b"));
    assert!(path.exists());
}

#[test]
fn config_path_can_come_from_environment() {
    let fixture = Fixture::new(&[]);
    let mut command = Command::cargo_bin("ollama-shelf").unwrap();
    command
        .env(
            "OLLAMA_SHELF_CONFIG",
            fixture.dir.path().join("shelves.yaml"),
        )
        .args(["--hostname", "devbox", "list", "extra"])
        .assert()
        .success()
        .stdout(predicate::str::contains("thinker:8b"));
}

#[test]
fn which_names_the_model_for_a_task() {
    let fixture = Fixture::new(&[]);
    fixture
        .command()
        .args(["which", "fix", "the", "bug"])
        .assert()
        .success()
        .stdout("coder -> coder:7b\n");
    fixture
        .command()
        .args(["which", "embed", "these", "docs"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("\"embedder\""));
}

#[test]
fn pull_fetches_only_missing_models() {
    let fixture = Fixture::new(&["tiny:1b"]);
    fixture
        .command()
        .arg("pull")
        .assert()
        .success()
        .stdout("have   tiny:1b\npull   coder:7b\n");
    assert_eq!(fixture.calls(), "list\npull coder:7b\n");
}

#[test]
fn pull_all_covers_every_shelf() {
    let fixture = Fixture::new(&["tiny:1b", "coder:7b"]);
    fixture.command().args(["pull", "all"]).assert().success();
    assert_eq!(fixture.calls(), "list\npull thinker:8b\n");
}

#[test]
fn pull_reports_ollama_failure() {
    let fixture = Fixture::new(&[]);
    fixture.fail_ollama();
    fixture
        .command()
        .arg("pull")
        .assert()
        .failure()
        .stderr(predicate::str::contains("ollama list").and(predicate::str::contains("boom")));
}

#[test]
fn gc_with_clean_shelf_removes_nothing() {
    let fixture = Fixture::new(&["tiny:1b"]);
    fixture
        .command()
        .arg("gc")
        .assert()
        .success()
        .stdout(predicate::str::contains("shelf is clean"));
    assert_eq!(fixture.calls(), "list\n");
}

#[test]
fn gc_yes_removes_orphans() {
    let fixture = Fixture::new(&["tiny:1b", "zeta:1b", "alpha:2b"]);
    fixture
        .command()
        .args(["gc", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("  alpha:2b\n  zeta:1b\n"));
    assert_eq!(fixture.calls(), "list\nrm alpha:2b\nrm zeta:1b\n");
}

#[test]
fn gc_prompt_respects_answer() {
    let fixture = Fixture::new(&["orphan:1b"]);
    fixture
        .command()
        .arg("gc")
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing removed"));
    assert_eq!(fixture.calls(), "list\n");

    fixture
        .command()
        .arg("gc")
        .write_stdin("y\n")
        .assert()
        .success();
    assert_eq!(fixture.calls(), "list\nlist\nrm orphan:1b\n");
}

#[test]
fn size_reports_blob_store() {
    let fixture = Fixture::new(&["tiny:1b"]);
    fixture.command().arg("size").assert().success().stdout(
        predicate::str::contains("tiny:1b abc123")
            .and(predicate::str::contains("blob store").not()),
    );

    let blobs = fixture.dir.path().join("models/blobs");
    std::fs::create_dir_all(&blobs).unwrap();
    std::fs::write(blobs.join("sha256-1"), vec![0u8; 2048]).unwrap();
    fixture
        .command()
        .arg("size")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "actual blob store on disk: 2.0 KiB",
        ));
}

/// Serves one canned HTTP response per entry in `responses`, returning the
/// base URL and a handle yielding the request bodies it received.
fn serve(responses: Vec<(u16, &'static str)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut bodies = Vec::new();
        for (status, body) in responses {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream);
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("content-length")
                {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut request = vec![0; length];
            reader.read_exact(&mut request).unwrap();
            bodies.push(String::from_utf8(request).unwrap());
            write!(
                reader.get_mut(),
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
        bodies
    });
    (url, handle)
}

#[test]
fn bench_reports_throughput() {
    let (url, server) = serve(vec![(
        200,
        r#"{"eval_count":100,"eval_duration":2000000000,"prompt_eval_count":30,"prompt_eval_duration":100000000,"load_duration":400000000}"#,
    )]);
    Fixture::new(&[])
        .command()
        .env("OLLAMA_HOST_URL", &url)
        .args(["bench", "thinker:8b"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("gen   50.0 tok/s")
                .and(predicate::str::contains("prompt    300.0 tok/s"))
                .and(predicate::str::contains("TTFT    500 ms")),
        );
    let bodies = server.join().unwrap();
    assert!(bodies[0].contains("\"model\":\"thinker:8b\""));
}

#[test]
fn bench_shelf_counts_failures() {
    let (url, server) = serve(vec![
        (400, r#"{"error":"model \"tiny:1b\" not found"}"#),
        (500, "overloaded"),
    ]);
    Fixture::new(&[])
        .command()
        .env("OLLAMA_HOST_URL", &url)
        .args(["bench", "core"])
        .assert()
        .failure()
        .stdout(
            predicate::str::contains("tiny:1b                ERROR: model \"tiny:1b\" not found")
                .and(predicate::str::contains(
                    "coder:7b               ERROR: Ollama returned 500 Internal Server Error: overloaded",
                )),
        )
        .stderr(predicate::str::contains("2 benchmark(s) failed"));
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn bench_rejects_zero_durations() {
    let (url, server) = serve(vec![(
        200,
        r#"{"eval_count":1,"eval_duration":0,"prompt_eval_count":1,"prompt_eval_duration":1,"load_duration":0}"#,
    )]);
    Fixture::new(&[])
        .command()
        .env("OLLAMA_HOST_URL", &url)
        .args(["bench", "tiny:1b"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("zero timing duration"));
    server.join().unwrap();
}
