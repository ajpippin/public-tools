use crate::config::{self, Config};
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

const LIST_TEMPLATE: &str = "%(title)s [%(id)s]";

#[derive(Parser)]
#[command(
    name = "ytd",
    version = concat!(env!("YTD_GIT_TAG"), " ", env!("YTD_GIT_SHA")),
    about = "Download videos and audio with yt-dlp"
)]
struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "YAML configuration file (or YTD_CONFIG)"
    )]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Download videos or audio.
    Get {
        #[arg(
            long,
            value_name = "DIR",
            help = "Override the configured download directory"
        )]
        dir: Option<PathBuf>,
        #[arg(long, help = "Extract the audio only")]
        audio: bool,
        #[command(flatten)]
        target: Target,
    },
    /// List the videos in a playlist or channel without downloading them.
    List {
        #[command(flatten)]
        target: Target,
    },
}

#[derive(Args)]
struct Target {
    #[arg(value_name = "URL", required = true)]
    urls: Vec<String>,
    #[arg(
        last = true,
        value_name = "YT_DLP_ARG",
        help = "Passed to yt-dlp as is"
    )]
    passthrough: Vec<String>,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let path = match &cli.config {
        Some(path) => path.clone(),
        None => config::default_path()?,
    };
    config::ensure(&path)?;
    let settings = config::load(&path)?;
    let binary = &settings.yt_dlp_binary;
    let status = Command::new(binary)
        .args(arguments(&cli.command, &settings))
        .status()
        .with_context(|| format!("run {binary}"))?;
    if !status.success() {
        bail!("{binary} failed: {status}");
    }
    Ok(())
}

fn arguments(action: &Action, settings: &Config) -> Vec<OsString> {
    let (mut args, target): (Vec<OsString>, _) = match action {
        Action::Get { dir, audio, target } => {
            let dir = dir.as_ref().unwrap_or(&settings.download_dir);
            let mut args = vec![
                "--paths".into(),
                dir.into(),
                "--output".into(),
                (&settings.output_template).into(),
            ];
            if *audio {
                args.push("--extract-audio".into());
            }
            (args, target)
        }
        Action::List { target } => (
            vec![
                "--flat-playlist".into(),
                "--print".into(),
                LIST_TEMPLATE.into(),
            ],
            target,
        ),
    };
    args.extend(settings.extra_args.iter().map(Into::into));
    args.extend(target.passthrough.iter().map(Into::into));
    args.push("--".into());
    args.extend(target.urls.iter().map(Into::into));
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Config {
        Config {
            yt_dlp_binary: "yt-dlp".into(),
            download_dir: "/downloads".into(),
            output_template: "%(id)s.%(ext)s".into(),
            extra_args: vec!["--no-mtime".into()],
        }
    }

    fn built(args: &[&str]) -> Vec<String> {
        let cli = Cli::try_parse_from([&["ytd"], args].concat()).unwrap();
        arguments(&cli.command, &settings())
            .into_iter()
            .map(|arg| arg.into_string().unwrap())
            .collect()
    }

    #[test]
    fn get_uses_configured_settings() {
        assert_eq!(
            built(&["get", "https://example.com/a"]),
            [
                "--paths",
                "/downloads",
                "--output",
                "%(id)s.%(ext)s",
                "--no-mtime",
                "--",
                "https://example.com/a"
            ]
        );
    }

    #[test]
    fn get_options_override_and_extend_settings() {
        assert_eq!(
            built(&["get", "--dir", "/music", "--audio", "u1", "u2", "--", "-q"]),
            [
                "--paths",
                "/music",
                "--output",
                "%(id)s.%(ext)s",
                "--extract-audio",
                "--no-mtime",
                "-q",
                "--",
                "u1",
                "u2"
            ]
        );
    }

    #[test]
    fn list_prints_playlist_entries_without_download_options() {
        assert_eq!(
            built(&["list", "u1", "--", "-q"]),
            [
                "--flat-playlist",
                "--print",
                "%(title)s [%(id)s]",
                "--no-mtime",
                "-q",
                "--",
                "u1"
            ]
        );
    }

    #[test]
    fn requires_a_command_and_url() {
        for args in [
            &["ytd"][..],
            &["ytd", "u1"],
            &["ytd", "get"],
            &["ytd", "get", "--", "-q"],
            &["ytd", "list"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }

    #[test]
    fn list_has_no_download_options() {
        assert!(Cli::try_parse_from(["ytd", "list", "--audio", "u1"]).is_err());
    }
}
