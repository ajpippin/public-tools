use crate::config;
use crate::engine::{CreateOptions, Engine};
use crate::version;
use anyhow::{Result, bail};
use chrono::Local;
use clap::{Args, Parser, Subcommand};
use std::io;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "template",
    about = "Create files from reusable templates",
    disable_version_flag = true
)]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "YAML configuration file (or TEMPLATE_CONFIG)"
    )]
    config: Option<PathBuf>,
    #[arg(long, global = true, help = "Print version information")]
    version: bool,
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    create: CreateArgs,
}

#[derive(Args, Default)]
struct CreateArgs {
    #[arg(short = 't', long, value_name = "NAME")]
    template: Option<String>,
    #[arg(short, long, conflicts_with = "backup")]
    force: bool,
    #[arg(long, conflicts_with = "force")]
    backup: bool,
    #[arg(value_name = "TARGET")]
    targets: Vec<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Create the configuration file if it does not exist.
    Init,
    /// List available templates.
    List,
    /// Show substitution values.
    Show {
        substitution: Option<String>,
        #[arg(long, default_value = "example.txt")]
        target: PathBuf,
        #[arg(short = 't', long)]
        template: Option<String>,
    },
    /// Print version information.
    Version,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    if cli.version {
        println!("{}", version::text());
        return Ok(());
    }
    let path = match cli.config {
        Some(path) => path,
        None => config::default_path()?,
    };
    match cli.command {
        Some(Command::Init) => {
            if config::initialize(&path)? {
                println!("Created configuration: {}", path.display());
            } else {
                println!("Configuration already exists: {}", path.display());
            }
        }
        Some(Command::Version) => println!("{}", version::text()),
        Some(command) => {
            if config::ensure(&path)? {
                eprintln!("created default configuration: {}", path.display());
            }
            execute_command(command, config::load(&path)?)?;
        }
        None => {
            if cli.create.targets.is_empty() {
                bail!("at least one target is required");
            }
            if config::ensure(&path)? {
                eprintln!("created default configuration: {}", path.display());
            }
            let engine = Engine {
                config: config::load(&path)?,
                now: Local::now(),
            };
            let options = CreateOptions {
                template: cli.create.template,
                force: cli.create.force,
                backup: cli.create.backup,
            };
            let stdin = io::stdin();
            let mut input = stdin.lock();
            let stdout = io::stdout();
            let mut output = stdout.lock();
            for target in cli.create.targets {
                engine.create(&target, &options, &mut input, &mut output)?;
            }
        }
    }
    Ok(())
}

fn execute_command(command: Command, settings: config::Config) -> Result<()> {
    let engine = Engine {
        config: settings,
        now: Local::now(),
    };
    match command {
        Command::List => {
            println!("TEMPLATE                 LOCATION");
            for item in engine.templates()? {
                println!("{:<24} {}", item.name, item.path.display());
            }
        }
        Command::Show {
            substitution,
            target,
            template,
        } => {
            let template_path = template
                .map(|name| engine.find_template(&name))
                .transpose()?
                .unwrap_or_default();
            let lines = engine.show_substitutions(&template_path, &target);
            if let Some(substitution) = substitution {
                let needle = substitution.trim_matches('%').to_lowercase();
                let line = lines.into_iter().find(|line| {
                    line.split_whitespace()
                        .next()
                        .unwrap_or("")
                        .trim_matches('%')
                        .to_lowercase()
                        == needle
                });
                println!(
                    "{}",
                    line.ok_or_else(|| anyhow::anyhow!(
                        "substitution {substitution:?} is not defined"
                    ))?
                );
            } else {
                for line in lines {
                    println!("{line}");
                }
            }
        }
        Command::Init | Command::Version => unreachable!(),
    }
    Ok(())
}
