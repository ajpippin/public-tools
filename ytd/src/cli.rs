use crate::config::{self, Config};
use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, Parser, Subcommand};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

const LIST_TEMPLATE: &str = "%(title)s [%(id)s]";
const VIDEO_URL: &str = "https://www.youtube.com/watch?v=";
const PLAYLIST_URL: &str = "https://www.youtube.com/playlist?list=";
const COMMANDS: [&str; 4] = ["get", "init", "list", "help"];

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
        help = "TOML configuration file (or YTD_CONFIG)"
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
    /// Print a default configuration file.
    Init,
    /// List the videos in a playlist or channel without downloading them.
    List {
        #[arg(long, help = "Print a title and ID line per video instead of JSON")]
        text: bool,
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
    let args: Vec<OsString> = env::args_os().collect();
    // These have to work when the configuration is missing or broken.
    match Cli::try_parse_from(&args) {
        Ok(cli) if matches!(cli.command, Action::Init) => {
            print!("{}", config::STARTER);
            return Ok(());
        }
        Err(error) if !error.use_stderr() => error.exit(),
        _ => {}
    }
    let path = match config_argument(&args) {
        Some(path) => path,
        None => config::default_path()?,
    };
    if !path.exists() {
        bail!("{}", missing_config(&path));
    }
    let settings = config::load(&path)?;
    let cli = Cli::parse_from(protect_video_ids(expand_aliases(args, &settings.aliases)));
    if matches!(cli.command, Action::Init) {
        print!("{}", config::STARTER);
        return Ok(());
    }
    let binary = &settings.yt_dlp_binary;
    let status = Command::new(binary)
        .args(arguments(&cli.command, &settings)?)
        .envs(
            settings
                .env
                .iter()
                .filter(|(name, _)| env::var_os(name).is_none()),
        )
        .status()
        .with_context(|| format!("run {binary}"))?;
    if !status.success() {
        bail!("{binary} failed: {status}");
    }
    Ok(())
}

fn missing_config(path: &Path) -> String {
    let mkdir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map_or(String::new(), |dir| {
            format!("  mkdir -p \"{}\"\n", dir.display())
        });
    format!(
        "no configuration file at {path}\n\nCreate one with:\n{mkdir}  ytd init > \"{path}\"\n\n{help}",
        path = path.display(),
        help = Cli::command().render_help().to_string().trim_end(),
    )
}

// Aliases are in the configuration, so --config is found before clap runs.
fn config_argument(args: &[OsString]) -> Option<PathBuf> {
    let options = &args[..args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len())];
    options.iter().enumerate().skip(1).find_map(|(index, arg)| {
        if arg == "--config" {
            options.get(index + 1).map(PathBuf::from)
        } else {
            arg.to_str()?.strip_prefix("--config=").map(PathBuf::from)
        }
    })
}

// Git-style expansion: the longest key matching the leading words wins, a
// one-word key never hides a command, and an expansion is not expanded again.
fn expand_aliases(args: Vec<OsString>, aliases: &BTreeMap<String, String>) -> Vec<OsString> {
    let start = match args.get(1).and_then(|arg| arg.to_str()) {
        Some("--config") => 3,
        Some(arg) if arg.starts_with("--config=") => 2,
        _ => 1,
    }
    .min(args.len());
    let words: Vec<&str> = args[start..]
        .iter()
        .map_while(|arg| arg.to_str())
        .take_while(|arg| !arg.starts_with('-'))
        .collect();
    let best = aliases
        .iter()
        .map(|(key, value)| (key.split_whitespace().collect::<Vec<_>>(), value))
        .filter(|(key, _)| !key.is_empty() && words.starts_with(key))
        .filter(|(key, _)| key.len() > 1 || !COMMANDS.contains(&key[0]))
        .max_by_key(|(key, _)| key.len());
    let Some((key, expansion)) = best else {
        return args;
    };
    let expansion: Vec<OsString> = expansion.split_whitespace().map(Into::into).collect();
    let mut expanded = args[..start].to_vec();
    expanded.extend(splice(expansion, args[start + key.len()..].to_vec()));
    expanded
}

// An expansion with its own -- still takes URLs from the command line, so they
// go ahead of that --, and only one -- survives.
fn splice(mut expansion: Vec<OsString>, mut rest: Vec<OsString>) -> Vec<OsString> {
    let Some(separator) = expansion.iter().position(|arg| arg == "--") else {
        expansion.append(&mut rest);
        return expansion;
    };
    let passthrough = match rest.iter().position(|arg| arg == "--") {
        Some(index) => rest.drain(index..).skip(1).collect(),
        None => vec![],
    };
    let tail = expansion.split_off(separator);
    expansion.extend(rest);
    expansion.extend(tail);
    expansion.extend(passthrough);
    expansion
}

// A YouTube video ID can start with -, which clap would read as an option, so
// those become URLs. Anything after -- is left for yt-dlp.
fn protect_video_ids(args: Vec<OsString>) -> Vec<OsString> {
    let end = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    args.into_iter()
        .enumerate()
        .map(|(index, arg)| match arg.to_str() {
            Some(text) if index < end && is_dashed_video_id(text) => {
                format!("{VIDEO_URL}{text}").into()
            }
            _ => arg,
        })
        .collect()
}

fn is_dashed_video_id(arg: &str) -> bool {
    arg.len() == 11
        && arg.starts_with('-')
        && arg
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

// The playlist behind a YouTube URL, which may be a video's URL within it. A
// generated mix (RD...) has no playlist page, so it keeps its own URL.
fn playlist_url(url: &str) -> Option<String> {
    let (location, query) = url.split_once('?')?;
    let host = location.split_once("://")?.1.split('/').next()?;
    let youtube = ["youtube.com", "youtu.be"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")));
    if !youtube {
        return None;
    }
    query
        .split('#')
        .next()?
        .split('&')
        .find_map(|parameter| parameter.strip_prefix("list="))
        .filter(|id| !id.is_empty() && !id.starts_with("RD"))
        .map(|id| format!("{PLAYLIST_URL}{id}"))
}

fn arguments(action: &Action, settings: &Config) -> Result<Vec<OsString>> {
    let (mut args, target): (Vec<OsString>, _) = match action {
        Action::Get { dir, audio, target } => {
            // An alias reaches here without the shell having expanded it.
            let dir = match dir {
                Some(dir) => config::expand_home(dir.clone())?,
                None => settings.download_dir.clone(),
            };
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
        Action::Init => unreachable!("init runs no yt-dlp command"),
        Action::List { text, target } => {
            let mut args = vec!["--flat-playlist".into()];
            if *text {
                args.extend(["--print".into(), LIST_TEMPLATE.into()]);
            } else {
                args.push("--dump-single-json".into());
            }
            (args, target)
        }
    };
    args.extend(settings.extra_args.iter().map(Into::into));
    args.extend(target.passthrough.iter().map(Into::into));
    args.push("--".into());
    let list = matches!(action, Action::List { .. });
    for url in &target.urls {
        let playlist = if list { playlist_url(url) } else { None };
        args.push(playlist.unwrap_or_else(|| url.clone()).into());
    }
    Ok(args)
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
            aliases: BTreeMap::new(),
            env: BTreeMap::new(),
        }
    }

    fn built(args: &[&str]) -> Vec<String> {
        let cli = Cli::try_parse_from([&["ytd"], args].concat()).unwrap();
        arguments(&cli.command, &settings())
            .unwrap()
            .into_iter()
            .map(|arg| arg.into_string().unwrap())
            .collect()
    }

    fn expanded(args: &[&str], aliases: &[(&str, &str)]) -> Vec<String> {
        let args = [&["ytd"], args].concat().iter().map(Into::into).collect();
        let aliases = aliases
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        expand_aliases(args, &aliases)
            .into_iter()
            .skip(1)
            .map(|arg| arg.into_string().unwrap())
            .collect()
    }

    #[test]
    fn expands_alias_and_keeps_trailing_arguments() {
        let aliases = [("m", "get --audio")];
        assert_eq!(
            expanded(&["m", "u1", "--dir", "/d"], &aliases),
            ["get", "--audio", "u1", "--dir", "/d"]
        );
    }

    #[test]
    fn leaves_unmatched_arguments_alone() {
        let aliases = [("m", "get --audio")];
        assert_eq!(expanded(&[], &aliases), [""; 0]);
        assert_eq!(expanded(&["get", "m"], &aliases), ["get", "m"]);
        assert_eq!(expanded(&["--version"], &aliases), ["--version"]);
    }

    #[test]
    fn one_word_alias_never_hides_a_command() {
        for command in COMMANDS {
            assert_eq!(
                expanded(&[command, "u1"], &[(command, "x")]),
                [command, "u1"]
            );
        }
    }

    #[test]
    fn multi_word_alias_may_start_with_a_command() {
        let aliases = [("get  audio", "get --audio")];
        assert_eq!(
            expanded(&["get", "audio", "u1"], &aliases),
            ["get", "--audio", "u1"]
        );
        assert_eq!(expanded(&["get", "u1"], &aliases), ["get", "u1"]);
    }

    #[test]
    fn longest_alias_wins_and_flags_end_the_key() {
        let aliases = [("x", "get"), ("x y", "list")];
        assert_eq!(expanded(&["x", "y", "u1"], &aliases), ["list", "u1"]);
        assert_eq!(expanded(&["x", "u1"], &aliases), ["get", "u1"]);
        assert_eq!(expanded(&["x", "--", "y"], &aliases), ["get", "--", "y"]);
    }

    #[test]
    fn expansion_is_not_expanded_again() {
        let aliases = [("a", "b"), ("b", "get")];
        assert_eq!(expanded(&["a", "u1"], &aliases), ["b", "u1"]);
    }

    #[test]
    fn alias_may_follow_leading_config_option() {
        let aliases = [("m", "get")];
        assert_eq!(
            expanded(&["--config", "f", "m", "u1"], &aliases),
            ["--config", "f", "get", "u1"]
        );
        assert_eq!(
            expanded(&["--config=f", "m", "u1"], &aliases),
            ["--config=f", "get", "u1"]
        );
        assert_eq!(expanded(&["--config"], &aliases), ["--config"]);
    }

    #[test]
    fn urls_go_ahead_of_a_separator_in_the_expansion() {
        let aliases = [("hd", "get -- --format best")];
        assert_eq!(
            expanded(&["hd", "u1", "u2"], &aliases),
            ["get", "u1", "u2", "--", "--format", "best"]
        );
        assert_eq!(
            expanded(&["hd", "u1", "--", "-q"], &aliases),
            ["get", "u1", "--", "--format", "best", "-q"]
        );
    }

    fn protected(args: &[&str]) -> Vec<String> {
        protect_video_ids(args.iter().map(Into::into).collect())
            .into_iter()
            .map(|arg| arg.into_string().unwrap())
            .collect()
    }

    #[test]
    fn dashed_video_ids_become_urls() {
        assert_eq!(
            protected(&["ytd", "get", "-jAq6qRM4nk", "--a_b-c_d-e"]),
            [
                "ytd",
                "get",
                "https://www.youtube.com/watch?v=-jAq6qRM4nk",
                "https://www.youtube.com/watch?v=--a_b-c_d-e"
            ]
        );
    }

    #[test]
    fn other_arguments_are_not_treated_as_video_ids() {
        for args in [
            &["ytd", "get", "--audio", "--dir", "/d", "V0pxDS3dpWg"][..],
            &["ytd", "get", "-abcdefghi"],
            &["ytd", "get", "-abcdefghijk"],
            &["ytd", "get", "-abcdefghi!"],
            &["ytd", "get", "u1", "--", "-jAq6qRM4nk"],
        ] {
            assert_eq!(protected(args), args);
        }
    }

    #[test]
    fn extracts_playlist_from_youtube_urls() {
        let expected = Some(format!("{PLAYLIST_URL}PLabc-d_e"));
        for url in [
            "https://www.youtube.com/watch?v=x1&list=PLabc-d_e&pp=y",
            "https://www.youtube.com/watch?list=PLabc-d_e",
            "https://www.youtube.com/playlist?list=PLabc-d_e",
            "https://music.youtube.com/watch?v=x1&list=PLabc-d_e#top",
            "https://youtu.be/x1?list=PLabc-d_e",
            "http://m.youtube.com/watch?v=x1&list=PLabc-d_e",
        ] {
            assert_eq!(playlist_url(url), expected, "{url}");
        }
    }

    #[test]
    fn leaves_urls_without_a_youtube_playlist_alone() {
        for url in [
            "PLabc-d_e",
            "https://www.youtube.com/watch?v=x1",
            "https://www.youtube.com/watch?v=x1&list=",
            "https://www.youtube.com/watch?v=x1&list=RDx1",
            "https://www.youtube.com/watch?v=x1&playlist=PLa",
            "https://www.youtube.com/@channel",
            "https://example.com/watch?list=PLa",
            "https://notyoutube.com/watch?list=PLa",
        ] {
            assert_eq!(playlist_url(url), None, "{url}");
        }
    }

    #[test]
    fn only_list_extracts_the_playlist() {
        let url = "https://www.youtube.com/watch?v=x1&list=PLa";
        assert_eq!(built(&["get", url]).last().unwrap(), url);
        assert_eq!(
            built(&["list", url]).last().unwrap(),
            "https://www.youtube.com/playlist?list=PLa"
        );
    }

    #[test]
    fn missing_config_explains_how_to_create_one() {
        let message = missing_config(Path::new("/x/etc/ytd.toml"));
        assert!(message.starts_with("no configuration file at /x/etc/ytd.toml\n"));
        assert!(message.contains("  mkdir -p \"/x/etc\"\n"));
        assert!(message.contains("  ytd init > \"/x/etc/ytd.toml\"\n"));
        assert!(message.contains("Usage: ytd"));
        assert!(!missing_config(Path::new("ytd.toml")).contains("mkdir"));
    }

    #[test]
    fn finds_config_argument_before_parsing() {
        let found = |args: &[&str]| {
            let args: Vec<OsString> = [&["ytd"], args].concat().iter().map(Into::into).collect();
            config_argument(&args)
        };
        assert_eq!(found(&["--config", "f", "get"]), Some("f".into()));
        assert_eq!(found(&["get", "u1", "--config=f"]), Some("f".into()));
        assert_eq!(found(&["get", "u1"]), None);
        assert_eq!(found(&["--config"]), None);
        assert_eq!(found(&["get", "u1", "--", "--config", "f"]), None);
    }

    #[test]
    fn expands_home_in_dir_option() {
        let args = built(&["get", "--dir", "~/Music", "u1"]);
        assert_eq!(
            args[1],
            dirs::home_dir()
                .unwrap()
                .join("Music")
                .display()
                .to_string()
        );
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
    fn list_defaults_to_json_without_download_options() {
        assert_eq!(
            built(&["list", "u1", "--", "-q"]),
            [
                "--flat-playlist",
                "--dump-single-json",
                "--no-mtime",
                "-q",
                "--",
                "u1"
            ]
        );
    }

    #[test]
    fn list_text_prints_a_line_per_video() {
        assert_eq!(
            built(&["list", "--text", "u1"]),
            [
                "--flat-playlist",
                "--print",
                "%(title)s [%(id)s]",
                "--no-mtime",
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
