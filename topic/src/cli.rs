use crate::config::{self, Config};
use crate::store::{Store, is_encrypted, run_interactive, update_footer, validate_author};
use crate::version;
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};

#[derive(Parser)]
#[command(
    name = "topic",
    about = "Browse and maintain a numbered personal topic library",
    disable_version_flag = true
)]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "YAML configuration file (or TOPIC_CONFIG)"
    )]
    config: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        value_name = "DIR",
        help = "Override the configured topic directory"
    )]
    dir: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Write topics directly instead of using less"
    )]
    no_pager: bool,
    #[arg(long, global = true, help = "Print version information")]
    version: bool,
    #[command(subcommand)]
    command: Option<Action>,
    #[arg(value_name = "EXPRESSION")]
    expression: Vec<String>,
}

#[derive(Subcommand)]
enum Action {
    /// Create the configuration file if it does not exist.
    Init,
    /// List topics.
    List { section: Option<String> },
    /// Display one or more topics.
    #[command(visible_alias = "p")]
    Print {
        #[arg(required = true)]
        topics: Vec<String>,
    },
    /// Search topic metadata.
    #[command(visible_alias = "q")]
    Search {
        #[arg(long = "all", help = "Search complete topic contents")]
        full: bool,
        expression: Vec<String>,
    },
    /// Edit, commit, and push a topic.
    Edit { topic: String },
    /// List section topics.
    Sections,
    /// Print version information.
    Version,
}

pub fn run() -> Result<()> {
    let args = resolve_aliases(env::args_os().collect())?;
    let cli = Cli::parse_from(args);
    if cli.version {
        println!("{}", version::text());
        return Ok(());
    }
    let path = match cli.config {
        Some(path) => path,
        None => config::default_path()?,
    };
    match cli.command {
        Some(Action::Init) => {
            if config::initialize(&path)? {
                println!("Created configuration: {}", path.display());
            } else {
                println!("Configuration already exists: {}", path.display());
            }
        }
        Some(Action::Version) => println!("{}", version::text()),
        action => {
            let mut settings = config::load(&path)?;
            if let Some(dir) = cli.dir {
                settings.topic_dir = config::expand_home(dir)?;
            }
            execute(action, cli.expression, settings, cli.no_pager)?;
        }
    }
    Ok(())
}

fn resolve_aliases(mut args: Vec<OsString>) -> Result<Vec<OsString>> {
    let config_index = args.iter().position(|arg| arg == "--config");
    let path = config_index
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .map_or_else(config::default_path, Ok)?;
    let initializing = args.iter().any(|argument| argument == "init");
    if !initializing && config::ensure(&path)? {
        eprintln!("created default configuration: {}", path.display());
    }
    let aliases = config::load(&path)
        .map(|c| c.aliases)
        .unwrap_or_else(|_| config::default_aliases());
    let mut command_index = None;
    let mut index = 1;
    while index < args.len() {
        let argument = args[index].to_string_lossy();
        if matches!(argument.as_ref(), "--config" | "--dir") {
            index += 2;
            continue;
        }
        if argument.starts_with('-') {
            index += 1;
            continue;
        }
        command_index = Some(index);
        break;
    }
    if let Some(index) = command_index
        && let Some(target) = aliases.get(&args[index].to_string_lossy().into_owned())
    {
        args[index] = target.into();
    }
    Ok(args)
}

fn execute(
    action: Option<Action>,
    expressions: Vec<String>,
    config: Config,
    no_pager: bool,
) -> Result<()> {
    let store = Store {
        dir: config.topic_dir.clone(),
        gpg_binary: config.gpg_binary.clone(),
    };
    match action {
        Some(Action::List { section }) => write_lines(store.list(section.as_deref(), false)?),
        Some(Action::Sections) => write_lines(store.list(None, true)?),
        Some(Action::Print { topics }) => display(&store, &topics, no_pager),
        Some(Action::Search { full, expression }) => search(&store, &expression, full, no_pager),
        Some(Action::Edit { topic }) => edit(&store, &config, &topic),
        None => search(&store, &expressions, false, no_pager),
        Some(Action::Init | Action::Version) => unreachable!(),
    }
}

fn search(store: &Store, terms: &[String], full: bool, no_pager: bool) -> Result<()> {
    let results = store.search(terms, full)?;
    if results.files.len() == 1 {
        return display(
            store,
            &[crate::store::topic_stem(&results.files[0]).unwrap()],
            no_pager,
        );
    }
    write_lines(results.lines)
}

fn display(store: &Store, topics: &[String], no_pager: bool) -> Result<()> {
    let mut content = String::new();
    for topic in topics {
        content.push_str(&store.read_topic(&store.find(topic)?)?);
    }
    if !no_pager && io::stdout().is_terminal() && command_exists("less") {
        let mut pager = Process::new("less")
            .args(["--quit-at-eof", "--no-init"])
            .stdin(Stdio::piped())
            .spawn()?;
        pager.stdin.take().unwrap().write_all(content.as_bytes())?;
        if pager.wait()?.success() {
            return Ok(());
        }
    }
    print!("{content}");
    Ok(())
}

fn edit(store: &Store, config: &Config, value: &str) -> Result<()> {
    validate_author(&config.author_name, &config.author_email)?;
    let path = store.find(value)?;
    if is_encrypted(&path) {
        if config.gpg_recipients.is_empty() {
            bail!("gpg_recipients is required to edit encrypted topics");
        }
        let mut temp = tempfile::NamedTempFile::new()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp.as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        temp.write_all(&store.read_data(&path)?)?;
        run_editor(temp.path())?;
        update_footer(temp.path(), &config.author_name, &config.author_email)?;
        store.encrypt(temp.path(), &path, &config.gpg_recipients)?;
        println!("Updated encrypted topic locally: {}", path.display());
        println!("Encrypted topics are not staged or committed to Git.");
        return Ok(());
    }
    run_editor(&path)?;
    update_footer(&path, &config.author_name, &config.author_email)?;
    let relative = path
        .file_name()
        .and_then(|v| v.to_str())
        .context("invalid topic filename")?;
    run_interactive(&store.dir, "git", &["add", "--", relative])?;
    let changed = Process::new("git")
        .args(["diff", "--cached", "--quiet", "--", relative])
        .current_dir(&store.dir)
        .status()?;
    if changed.success() {
        println!("No changes to commit.");
        return Ok(());
    }
    if changed.code() != Some(1) {
        bail!("check staged topic: git exited with {changed}");
    }
    let stem = Path::new(relative)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    run_interactive(
        &store.dir,
        "git",
        &[
            "commit",
            "-m",
            &format!("Update topic {stem}"),
            "--",
            relative,
        ],
    )?;
    println!("Pushing to Git...");
    run_interactive(&store.dir, "git", &["push", "origin"])
}

fn run_editor(path: &Path) -> Result<()> {
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let status = Process::new(program)
        .args(parts)
        .arg(path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("editor")?;
    if !status.success() {
        bail!("editor exited with {status}");
    }
    Ok(())
}

fn command_exists(name: &str) -> bool {
    env::var_os("PATH")
        .is_some_and(|paths| env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}
fn write_lines(lines: Vec<String>) -> Result<()> {
    for line in lines {
        println!("{line}");
    }
    Ok(())
}
