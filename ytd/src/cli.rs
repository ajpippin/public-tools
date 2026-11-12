use crate::config::{self, Config};
use crate::transcript;
use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LIST_TEMPLATE: &str = "%(title)s [%(id)s]";
const VIDEO_URL: &str = "https://www.youtube.com/watch?v=";
const PLAYLIST_URL: &str = "https://www.youtube.com/playlist?list=";
const COMMANDS: [&str; 4] = ["get", "init", "list", "help"];
// Under scratch, the videos that yt-dlp saved, for extracting their audio.
const VIDEOS: &str = "videos";

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
    /// Download videos, audio, or transcripts.
    Get {
        #[arg(
            long,
            value_name = "DIR",
            help = "Override the configured download directory"
        )]
        dir: Option<PathBuf>,
        #[arg(
            long = "type",
            value_name = "TYPES",
            value_delimiter = ',',
            help = "What to save: mp4, mp3, txt, or all, comma-separated [default: mp4]"
        )]
        types: Vec<Kind>,
        #[arg(long, help = "Save the audio as MP3, the same as --type mp3")]
        audio: bool,
        #[arg(long, help = "Save the captions as plain text, the same as --type txt")]
        transcript: bool,
        #[arg(
            long,
            value_name = "LANGS",
            help = "Transcript languages, as yt-dlp's --sub-langs takes them [default: en]"
        )]
        lang: Option<String>,
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

// The order here is the order of the downloads.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Kind {
    #[value(help = "The video, as MP4")]
    Mp4,
    #[value(help = "The audio, as MP3")]
    Mp3,
    #[value(help = "The captions, as plain text")]
    Txt,
    #[value(help = "All three")]
    All,
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
    let scratch = tempfile::tempdir().context("create temporary directory")?;
    let binary = &settings.yt_dlp_binary;
    for args in invocations(&cli.command, &settings, scratch.path())? {
        let status = Command::new(binary)
            .args(args)
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
    }
    let kinds = kinds(&cli.command)?;
    if extracts_audio(&kinds) {
        extract_audio(&settings.ffmpeg_binary, &scratch.path().join(VIDEOS))?;
    }
    if let Action::Get { dir, .. } = &cli.command
        && kinds.contains(&Kind::Txt)
    {
        for path in transcript::save(scratch.path(), &output_dir(dir, &settings)?)? {
            println!("{}", path.display());
        }
    }
    Ok(())
}

// With both the video and the audio wanted, the audio comes from the
// downloaded video rather than a second download.
fn extracts_audio(kinds: &[Kind]) -> bool {
    kinds.contains(&Kind::Mp4) && kinds.contains(&Kind::Mp3)
}

// Saves the audio of each video listed in `videos` as MP3 beside it, with the
// quality yt-dlp's --audio-format mp3 gives by default.
fn extract_audio(ffmpeg: &str, videos: &Path) -> Result<()> {
    let list = fs::read_to_string(videos).unwrap_or_default();
    let mut paths: Vec<&str> = list.lines().filter(|line| !line.is_empty()).collect();
    paths.dedup();
    if paths.is_empty() {
        bail!("no video downloaded to extract the audio from");
    }
    for video in paths {
        let audio = Path::new(video).with_extension("mp3");
        let status = Command::new(ffmpeg)
            .args(["-nostdin", "-loglevel", "error", "-y", "-i"])
            .arg(video)
            .args(["-vn", "-codec:a", "libmp3lame", "-q:a", "5"])
            .arg(&audio)
            .status()
            .with_context(|| format!("run {ffmpeg}"))?;
        if !status.success() {
            bail!("{ffmpeg} failed: {status}");
        }
        println!("{}", audio.display());
    }
    Ok(())
}

// What get saves, in download order. --audio and --transcript add to --type,
// and nothing chosen means the video.
fn kinds(action: &Action) -> Result<Vec<Kind>> {
    let Action::Get {
        types,
        audio,
        transcript,
        lang,
        ..
    } = action
    else {
        return Ok(vec![]);
    };
    let mut kinds: BTreeSet<Kind> = types
        .iter()
        .flat_map(|kind| match kind {
            Kind::All => vec![Kind::Mp4, Kind::Mp3, Kind::Txt],
            kind => vec![*kind],
        })
        .collect();
    if *audio {
        kinds.insert(Kind::Mp3);
    }
    if *transcript {
        kinds.insert(Kind::Txt);
    }
    if kinds.is_empty() {
        kinds.insert(Kind::Mp4);
    }
    if lang.is_some() && !kinds.contains(&Kind::Txt) {
        bail!("--lang needs --type txt or --transcript");
    }
    Ok(kinds.into_iter().collect())
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

fn output_dir(dir: &Option<PathBuf>, settings: &Config) -> Result<PathBuf> {
    match dir {
        // An alias reaches here without the shell having expanded it.
        Some(dir) => config::expand_home(dir.clone()),
        None => Ok(settings.download_dir.clone()),
    }
}

// The yt-dlp arguments for each run: one for list, and one per kind for get.
fn invocations(action: &Action, settings: &Config, scratch: &Path) -> Result<Vec<Vec<OsString>>> {
    let (runs, target): (Vec<Vec<OsString>>, _) = match action {
        Action::Get {
            dir, lang, target, ..
        } => {
            let dir = output_dir(dir, settings)?;
            let kinds = kinds(action)?;
            let extract = extracts_audio(&kinds);
            let runs = kinds
                .into_iter()
                .filter(|kind| !(extract && *kind == Kind::Mp3))
                .map(|kind| {
                    let mut args = get_arguments(kind, &dir, lang, settings, scratch);
                    if extract && kind == Kind::Mp4 {
                        args.extend([
                            "--print-to-file".into(),
                            "after_move:filepath".into(),
                            scratch.join(VIDEOS).into(),
                        ]);
                    }
                    args
                })
                .collect();
            (runs, target)
        }
        Action::Init => unreachable!("init runs no yt-dlp command"),
        Action::List { text, target } => {
            let mut args = vec!["--flat-playlist".into()];
            if *text {
                args.extend(["--print".into(), LIST_TEMPLATE.into()]);
            } else {
                args.push("--dump-single-json".into());
            }
            (vec![args], target)
        }
    };
    let list = matches!(action, Action::List { .. });
    Ok(runs
        .into_iter()
        .map(|mut args| {
            args.extend(settings.extra_args.iter().map(Into::into));
            args.extend(target.passthrough.iter().map(Into::into));
            args.push("--".into());
            for url in &target.urls {
                let playlist = if list { playlist_url(url) } else { None };
                args.push(playlist.unwrap_or_else(|| url.clone()).into());
            }
            args
        })
        .collect())
}

fn get_arguments(
    kind: Kind,
    dir: &Path,
    lang: &Option<String>,
    settings: &Config,
    scratch: &Path,
) -> Vec<OsString> {
    // A transcript is converted from captions kept in scratch.
    let dir = if kind == Kind::Txt { scratch } else { dir };
    let mut args = vec![
        "--paths".into(),
        dir.into(),
        "--output".into(),
        (&settings.output_template).into(),
        "--no-playlist".into(),
    ];
    match kind {
        Kind::Mp4 => args.extend([
            "--merge-output-format".into(),
            "mp4".into(),
            "--remux-video".into(),
            "mp4".into(),
        ]),
        Kind::Mp3 => args.extend([
            "--extract-audio".into(),
            "--audio-format".into(),
            "mp3".into(),
        ]),
        Kind::Txt => args.extend([
            "--skip-download".into(),
            "--write-subs".into(),
            "--write-auto-subs".into(),
            "--sub-langs".into(),
            lang.as_deref().unwrap_or("en").into(),
            "--convert-subs".into(),
            "srt".into(),
        ]),
        Kind::All => unreachable!("all is split into its kinds"),
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Config {
        Config {
            yt_dlp_binary: "yt-dlp".into(),
            ffmpeg_binary: "ffmpeg".into(),
            download_dir: "/downloads".into(),
            output_template: "%(id)s.%(ext)s".into(),
            extra_args: vec!["--no-mtime".into()],
            aliases: BTreeMap::new(),
            env: BTreeMap::new(),
        }
    }

    fn runs(args: &[&str]) -> Result<Vec<Vec<String>>> {
        let cli = Cli::try_parse_from([&["ytd"], args].concat()).unwrap();
        Ok(
            invocations(&cli.command, &settings(), Path::new("/scratch"))?
                .into_iter()
                .map(|run| {
                    run.into_iter()
                        .map(|arg| arg.into_string().unwrap())
                        .collect()
                })
                .collect(),
        )
    }

    fn built(args: &[&str]) -> Vec<String> {
        let mut runs = runs(args).unwrap();
        assert_eq!(runs.len(), 1);
        runs.remove(0)
    }

    // The option that sets each run apart, such as --remux-video for mp4, or
    // --print-to-file for mp4 with the audio extracted from it.
    fn kinds_run(args: &[&str]) -> Vec<&'static str> {
        runs(args)
            .unwrap()
            .iter()
            .map(|run| {
                [
                    "--print-to-file",
                    "--remux-video",
                    "--extract-audio",
                    "--skip-download",
                ]
                .into_iter()
                .find(|option| run.iter().any(|arg| arg == option))
                .unwrap()
            })
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
                "--no-playlist",
                "--merge-output-format",
                "mp4",
                "--remux-video",
                "mp4",
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
                "--no-playlist",
                "--extract-audio",
                "--audio-format",
                "mp3",
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
    fn get_transcript_fetches_captions_into_scratch_without_the_video() {
        assert_eq!(
            built(&[
                "get",
                "--transcript",
                "--lang",
                "fr",
                "--dir",
                "/d",
                "u1",
                "--",
                "-q"
            ]),
            [
                "--paths",
                "/scratch",
                "--output",
                "%(id)s.%(ext)s",
                "--no-playlist",
                "--skip-download",
                "--write-subs",
                "--write-auto-subs",
                "--sub-langs",
                "fr",
                "--convert-subs",
                "srt",
                "--no-mtime",
                "-q",
                "--",
                "u1"
            ]
        );
        assert!(built(&["get", "--transcript", "u1"]).contains(&"en".to_owned()));
    }

    #[test]
    fn get_without_transcript_requests_no_captions() {
        assert!(!built(&["get", "u1"]).contains(&"--write-subs".to_owned()));
    }

    #[test]
    fn transcript_options_are_checked() {
        assert!(Cli::try_parse_from(["ytd", "list", "--transcript", "u1"]).is_err());
        assert!(Cli::try_parse_from(["ytd", "list", "--type", "mp3", "u1"]).is_err());
        assert!(Cli::try_parse_from(["ytd", "get", "--type", "mkv", "u1"]).is_err());
        assert!(runs(&["get", "--lang", "fr", "u1"]).is_err());
        assert!(runs(&["get", "--type", "mp3", "--lang", "fr", "u1"]).is_err());
        assert!(runs(&["get", "--type", "txt", "--lang", "fr", "u1"]).is_ok());
    }

    #[test]
    fn video_run_lists_its_files_when_the_audio_is_extracted() {
        let run = &runs(&["get", "--type", "mp4,mp3", "u1"]).unwrap()[0];
        let at = run.iter().position(|arg| arg == "--print-to-file").unwrap();
        assert_eq!(run[at + 1], "after_move:filepath");
        assert_eq!(run[at + 2], "/scratch/videos");
        assert!(!run.contains(&"--extract-audio".into()));
    }

    #[test]
    fn type_chooses_what_get_saves() {
        let video = "--remux-video";
        let both = "--print-to-file";
        let audio = "--extract-audio";
        let text = "--skip-download";
        assert_eq!(kinds_run(&["get", "u1"]), [video]);
        assert_eq!(kinds_run(&["get", "--type", "mp3", "u1"]), [audio]);
        assert_eq!(kinds_run(&["get", "--type=txt", "u1"]), [text]);
        assert_eq!(
            kinds_run(&["get", "--type", "txt,mp4", "u1"]),
            [video, text]
        );
        assert_eq!(
            kinds_run(&["get", "--type", "mp3", "--type", "mp4", "u1"]),
            [both]
        );
        assert_eq!(kinds_run(&["get", "--type", "all", "u1"]), [both, text]);
        assert_eq!(kinds_run(&["get", "--type", "all,mp3", "u1"]), [both, text]);
        assert_eq!(
            kinds_run(&["get", "--audio", "--transcript", "u1"]),
            [audio, text]
        );
        assert_eq!(
            kinds_run(&["get", "--type", "mp4", "--audio", "u1"]),
            [both]
        );
    }

    #[test]
    fn each_run_downloads_every_url_with_the_passthrough() {
        for run in runs(&[
            "get", "--type", "all", "--dir", "/d", "u1", "u2", "--", "-q",
        ])
        .unwrap()
        {
            assert!(run.ends_with(&["-q".into(), "--".into(), "u1".into(), "u2".into()]));
            assert_eq!(
                run[1],
                if run.contains(&"--skip-download".into()) {
                    "/scratch"
                } else {
                    "/d"
                }
            );
        }
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
        assert!(Cli::try_parse_from(["ytd", "list", "--lang", "en", "u1"]).is_err());
    }
}
