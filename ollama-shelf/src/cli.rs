use crate::config::{self, HostShelf, Model};
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(
    name = "ollama-shelf",
    about = "Curate and operate a host-specific Ollama model shelf"
)]
struct Cli {
    #[arg(long, global = true, value_name = "FILE", help = "Shelf YAML file")]
    config: Option<PathBuf>,
    #[arg(long, global = true, help = "Hostname profile to use")]
    hostname: Option<String>,
    #[command(subcommand)]
    command: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Create a starter configuration.
    Init,
    /// List one shelf or all configured shelves.
    List {
        #[arg(default_value = "all")]
        shelf: String,
    },
    /// Pull missing shelf models.
    Pull {
        #[arg(default_value = "core")]
        shelf: String,
    },
    /// Show model and blob-store sizes.
    Size,
    /// Remove local models outside the shelf.
    Gc {
        #[arg(short, long)]
        yes: bool,
    },
    /// Benchmark a model, a shelf, or all configured models.
    Bench { target: String },
    /// Choose a model for a task.
    Which {
        #[arg(required = true, num_args = 1..)]
        task: Vec<String>,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = match cli.config {
        Some(path) => path,
        None => config::configured_path()?,
    };
    let hostname = match cli.hostname {
        Some(hostname) => hostname,
        None => hostname::get()
            .context("get hostname")?
            .to_string_lossy()
            .into_owned(),
    };
    execute(cli.command, &config, &hostname)
}

fn execute(action: Action, path: &Path, hostname: &str) -> Result<()> {
    if matches!(action, Action::Init) {
        config::initialize(path, hostname)?;
        println!("created {}", path.display());
        return Ok(());
    }
    if config::ensure(path, hostname)? {
        eprintln!("created default configuration: {}", path.display());
    }
    let shelf = config::load(path, hostname)?;
    match action {
        Action::Init => unreachable!(),
        Action::List { shelf: requested } => list(&shelf, &requested),
        Action::Pull { shelf: requested } => pull(&shelf, &requested),
        Action::Size => size(&shelf),
        Action::Gc { yes } => gc(&shelf, yes),
        Action::Bench { target } => bench(&shelf, &target),
        Action::Which { task } => which(&shelf, &task.join(" ")),
    }
}

fn list(shelf: &HostShelf, requested: &str) -> Result<()> {
    if requested == "all" {
        for (index, (name, models)) in shelf.shelves.iter().enumerate() {
            if index > 0 {
                println!();
            }
            print_models(&name.to_uppercase(), models);
        }
    } else {
        print_models(
            &requested.to_uppercase(),
            shelf
                .shelves
                .get(requested)
                .with_context(|| format!("shelf {requested:?} is not configured"))?,
        );
    }
    Ok(())
}

fn print_models(title: &str, models: &[Model]) {
    println!("== {title} ==");
    println!("{:<22} | {:<10} | PURPOSE", "MODEL", "SLOT");
    println!("{:-<22}-+-{:-<10}-+-{:-<32}", "", "", "");
    for model in models {
        println!(
            "{:<22} | {:<10} | {}",
            model.name, model.slot, model.purpose
        );
    }
}

fn pull(shelf: &HostShelf, requested: &str) -> Result<()> {
    let present = local_models()?;
    for model in shelf.models(requested)? {
        if present.contains(&model.name) {
            println!("have   {}", model.name);
        } else {
            println!("pull   {}", model.name);
            run_ollama(&["pull", &model.name]).with_context(|| format!("pull {}", model.name))?;
        }
    }
    Ok(())
}

fn size(shelf: &HostShelf) -> Result<()> {
    let output = command_output(&["list"]).context("ollama list")?;
    println!("model sizes (as reported by 'ollama list'):");
    print!("{output}");
    let blobs = Path::new(&shelf.models_path).join("blobs");
    match directory_size(&blobs) {
        Ok(bytes) => println!("\nactual blob store on disk: {}", human_bytes(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("measure {:?}", blobs)),
    }
    Ok(())
}

fn gc(shelf: &HostShelf, yes: bool) -> Result<()> {
    let keep: HashSet<_> = shelf
        .all_models()
        .into_iter()
        .map(|model| model.name)
        .collect();
    let mut orphans: Vec<_> = local_models()?.difference(&keep).cloned().collect();
    orphans.sort();
    println!("Local models not on your curated shelf:");
    if orphans.is_empty() {
        println!("  (none -- shelf is clean)");
        return Ok(());
    }
    for model in &orphans {
        println!("  {model}");
    }
    if !yes {
        print!("\nRemove all of the above? [y/N] ");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().lock().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y") {
            println!("nothing removed");
            return Ok(());
        }
    }
    for model in orphans {
        run_ollama(&["rm", &model]).with_context(|| format!("remove {model}"))?;
    }
    Ok(())
}

fn which(shelf: &HostShelf, task: &str) -> Result<()> {
    let slot = route_slot(&task.to_lowercase());
    if let Some(model) = shelf
        .all_models()
        .into_iter()
        .find(|model| model.slot.trim_end_matches('+') == slot)
    {
        println!("{} -> {}", model.slot, model.name);
        Ok(())
    } else {
        bail!("task maps to slot {slot:?}, but that slot is not configured")
    }
}

fn route_slot(task: &str) -> &'static str {
    let rules: &[(&str, &[&str])] = &[
        ("embedder", &["embed", "similarity", "vector", "semantic"]),
        (
            "coder",
            &[
                "refactor",
                "write a",
                "implement",
                "fix the bug",
                "debug",
                "code",
                "function",
                "regex",
            ],
        ),
        (
            "tiny",
            &[
                "classif",
                "route",
                "yes or no",
                "label",
                "one word",
                "autocomplete",
            ],
        ),
        (
            "reasoner",
            &[
                "reason",
                "prove",
                "step by step",
                "think through",
                "math",
                "puzzle",
            ],
        ),
        (
            "workhorse",
            &["summar", "rag", "answer from", "tool", "chat", "explain"],
        ),
    ];
    rules
        .iter()
        .find(|(_, words)| words.iter().any(|word| task.contains(word)))
        .map_or("workhorse", |(slot, _)| slot)
}

fn local_models() -> Result<HashSet<String>> {
    let output = command_output(&["list"]).context("ollama list")?;
    Ok(output
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect())
}

fn command_output(args: &[&str]) -> Result<String> {
    let output = Command::new("ollama")
        .args(args)
        .output()
        .context("run ollama")?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() {
        Ok(combined)
    } else {
        bail!("{combined}")
    }
}

fn run_ollama(args: &[&str]) -> Result<()> {
    let status = Command::new("ollama")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("run ollama")?;
    if status.success() {
        Ok(())
    } else {
        bail!("ollama exited with {status}")
    }
}

#[derive(Deserialize)]
struct BenchmarkResponse {
    eval_count: u64,
    eval_duration: u64,
    prompt_eval_count: u64,
    prompt_eval_duration: u64,
    load_duration: u64,
}

fn bench(shelf: &HostShelf, target: &str) -> Result<()> {
    println!(
        "{:<22} {:<18} {:<20} TTFT MS",
        "MODEL", "GEN TOK/S", "PROMPT TOK/S"
    );
    let client = Client::builder()
        .timeout(Duration::from_secs(180))
        .build()?;
    let mut failures = 0;
    for model in shelf.benchmark_targets(target)? {
        if let Err(error) = benchmark_one(&client, &shelf.ollama_host_url, &model) {
            println!("{model:<22} ERROR: {error}");
            failures += 1;
        }
    }
    if failures > 0 {
        bail!("{failures} benchmark(s) failed");
    }
    Ok(())
}

fn benchmark_one(client: &Client, base_url: &str, model: &str) -> Result<()> {
    let response = client.post(format!("{}/api/generate", base_url.trim_end_matches('/')))
        .json(&json!({"model": model, "prompt": "List five uses for a hash set.", "stream": false, "options": {"num_predict": 120, "temperature": 0}}))
        .send().with_context(|| format!("benchmark {model}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        if status.as_u16() == 400
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&body)
            && let Some(message) = value.get("error").and_then(|value| value.as_str())
        {
            bail!("{message}");
        }
        bail!("Ollama returned {status}: {}", body.trim());
    }
    let data: BenchmarkResponse = response.json()?;
    if data.eval_duration == 0 || data.prompt_eval_duration == 0 {
        bail!("benchmark {model}: Ollama returned zero timing duration");
    }
    let generated = data.eval_count as f64 / (data.eval_duration as f64 / 1e9);
    let prompt = data.prompt_eval_count as f64 / (data.prompt_eval_duration as f64 / 1e9);
    let ttft = (data.load_duration + data.prompt_eval_duration) as f64 / 1e6;
    println!(
        "{model:<22} gen {generated:6.1} tok/s   prompt {prompt:8.1} tok/s   TTFT {ttft:6.0} ms"
    );
    Ok(())
}

fn directory_size(path: &Path) -> io::Result<u64> {
    let mut total = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            total += directory_size(&entry.path())?;
        } else if metadata.is_file() {
            total += metadata.len();
        }
    }
    Ok(total)
}

fn human_bytes(bytes: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut index = 0;
    while size >= 1024.0 && index < units.len() - 1 {
        size /= 1024.0;
        index += 1;
    }
    if index == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", units[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_tasks() {
        for (task, expected) in [
            ("summarize a PDF", "workhorse"),
            ("implement a function", "coder"),
            ("vector similarity", "embedder"),
            ("prove this", "reasoner"),
            ("one word label", "tiny"),
        ] {
            assert_eq!(route_slot(&task.to_lowercase()), expected);
        }
    }

    #[test]
    fn formats_bytes() {
        assert_eq!(human_bytes(500), "500 B");
        assert_eq!(human_bytes(1536), "1.5 KiB");
    }
}
