use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub const STARTER: &str = "# ytd configuration. Uncomment and edit values to override defaults.\n\
# A variable under [env] is used only if it is not already set in the environment.\n\
# yt_dlp_binary = \"yt-dlp\"\n\
# ffmpeg_binary = \"ffmpeg\"\n\
# download_dir = \"~/Downloads\"\n\
# output_template = \"%(title)s [%(id)s].%(ext)s\"\n\
# extra_args = []\n\
# [aliases]\n\
# music = \"get --audio --dir ~/Music\"\n\
# [env]\n\
# HTTPS_PROXY = \"http://proxy.example.com:8080\"\n";

#[derive(Clone, Debug)]
pub struct Config {
    pub yt_dlp_binary: String,
    pub ffmpeg_binary: String,
    pub download_dir: PathBuf,
    pub output_template: String,
    pub extra_args: Vec<String>,
    pub aliases: BTreeMap<String, String>,
    pub env: BTreeMap<String, String>,
}

pub fn default_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("YTD_CONFIG").filter(|value| !value.is_empty()) {
        return Ok(path.into());
    }
    path_in(env::var_os("XDG_CONFIG_HOME"), dirs::home_dir())
}

// The first of these that exists, or the first when none does:
// $XDG_CONFIG_HOME/tools, ~/.config/tools, $XDG_CONFIG_HOME, then ~/.config.
fn path_in(xdg_config_home: Option<OsString>, home: Option<PathBuf>) -> Result<PathBuf> {
    // The XDG specification says to ignore a relative XDG_CONFIG_HOME.
    let xdg = xdg_config_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let bases: Vec<PathBuf> = [xdg, home.map(|home| home.join(".config"))]
        .into_iter()
        .flatten()
        .collect();
    if bases.is_empty() {
        bail!("cannot determine home directory");
    }
    let mut paths: Vec<PathBuf> = bases
        .iter()
        .map(|base| base.join("tools/ytd.toml"))
        .collect();
    paths.extend(bases.iter().map(|base| base.join("ytd.toml")));
    let found = paths.iter().position(|path| path.exists()).unwrap_or(0);
    Ok(paths.swap_remove(found))
}

pub fn defaults() -> Config {
    Config {
        yt_dlp_binary: "yt-dlp".into(),
        ffmpeg_binary: "ffmpeg".into(),
        download_dir: dirs::home_dir().unwrap_or_default().join("Downloads"),
        output_template: "%(title)s [%(id)s].%(ext)s".into(),
        extra_args: vec![],
        aliases: BTreeMap::new(),
        env: BTreeMap::new(),
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    yt_dlp_binary: Option<String>,
    ffmpeg_binary: Option<String>,
    download_dir: Option<PathBuf>,
    output_template: Option<String>,
    extra_args: Option<Vec<String>>,
    aliases: Option<BTreeMap<String, String>>,
    env: Option<BTreeMap<String, String>>,
}

pub fn load(path: &Path) -> Result<Config> {
    let mut settings = defaults();
    let text = fs::read_to_string(path).with_context(|| format!("read config {:?}", path))?;
    let file: ConfigFile =
        toml::from_str(&text).with_context(|| format!("parse config {:?}", path))?;
    if let Some(v) = file.yt_dlp_binary {
        settings.yt_dlp_binary = v;
    }
    if let Some(v) = file.ffmpeg_binary {
        settings.ffmpeg_binary = v;
    }
    if let Some(v) = file.download_dir {
        settings.download_dir = v;
    }
    if let Some(v) = file.output_template {
        settings.output_template = v;
    }
    if let Some(v) = file.extra_args {
        settings.extra_args = v;
    }
    if let Some(v) = file.aliases {
        settings.aliases = v;
    }
    if let Some(v) = file.env {
        settings.env = v;
    }
    if settings.yt_dlp_binary.trim().is_empty() {
        bail!("config {:?} has an empty yt_dlp_binary", path);
    }
    if settings.ffmpeg_binary.trim().is_empty() {
        bail!("config {:?} has an empty ffmpeg_binary", path);
    }
    if settings.download_dir.as_os_str().is_empty() {
        bail!("config {:?} has an empty download_dir", path);
    }
    if settings.output_template.trim().is_empty() {
        bail!("config {:?} has an empty output_template", path);
    }
    for (name, value) in &settings.env {
        if name.is_empty() || name.contains(['=', '\0']) || value.contains('\0') {
            bail!(
                "config {:?} has invalid environment variable {name:?}",
                path
            );
        }
    }
    settings.download_dir = expand_home(settings.download_dir)?;
    Ok(settings)
}

pub fn expand_home(path: PathBuf) -> Result<PathBuf> {
    let text = path.to_string_lossy();
    if text == "~" {
        return dirs::home_dir().context("expand download_dir: cannot determine home directory");
    }
    if let Some(rest) = text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\")) {
        return Ok(dirs::home_dir()
            .context("expand download_dir: cannot determine home directory")?
            .join(rest));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_config(text: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, text).unwrap();
        (dir, path)
    }

    fn load_error(text: &str) -> String {
        let (_dir, path) = write_config(text);
        load(&path).unwrap_err().to_string()
    }

    #[test]
    fn path_lookup_prefers_tools_directories_and_xdg() {
        let dir = tempfile::tempdir().unwrap();
        let xdg = dir.path().join("xdg");
        let home = dir.path().join("home");
        let path = |xdg: Option<&Path>| {
            path_in(xdg.map(|xdg| xdg.as_os_str().into()), Some(home.clone())).unwrap()
        };
        assert_eq!(path(Some(&xdg)), xdg.join("tools/ytd.toml"));
        assert_eq!(path(None), home.join(".config/tools/ytd.toml"));
        for value in ["", "relative"] {
            assert_eq!(
                path_in(Some(value.into()), Some(home.clone())).unwrap(),
                home.join(".config/tools/ytd.toml")
            );
        }
        // Each file created outranks the ones before it.
        for expected in [
            home.join(".config/ytd.toml"),
            xdg.join("ytd.toml"),
            home.join(".config/tools/ytd.toml"),
            xdg.join("tools/ytd.toml"),
        ] {
            fs::create_dir_all(expected.parent().unwrap()).unwrap();
            fs::write(&expected, "").unwrap();
            assert_eq!(path(Some(&xdg)), expected);
        }
        assert_eq!(path(None), home.join(".config/tools/ytd.toml"));
        assert!(path_in(None, None).is_err());
        assert_eq!(
            path_in(Some(xdg.as_os_str().into()), None).unwrap(),
            xdg.join("tools/ytd.toml")
        );
    }

    #[test]
    fn empty_config_uses_defaults() {
        let (_dir, path) = write_config("");
        let config = load(&path).unwrap();
        assert_eq!(config.yt_dlp_binary, "yt-dlp");
        assert!(config.download_dir.ends_with("Downloads"));
        assert_eq!(config.output_template, defaults().output_template);
        assert!(config.aliases.is_empty());
        assert!(config.env.is_empty());
    }

    #[test]
    fn reports_missing_config() {
        let dir = tempfile::tempdir().unwrap();
        let error = load(&dir.path().join("missing.toml")).unwrap_err();
        assert!(error.to_string().starts_with("read config"));
    }

    #[test]
    fn loads_and_expands_config() {
        let (_dir, path) = write_config(
            "yt_dlp_binary = \"/opt/yt-dlp\"\ndownload_dir = \"~/video\"\noutput_template = \"%(id)s\"\nextra_args = [\"--no-mtime\"]\n[aliases]\nm = \"get --audio\"\n[env]\nA = \"1\"\nB = \"\"\n",
        );
        let config = load(&path).unwrap();
        assert_eq!(config.yt_dlp_binary, "/opt/yt-dlp");
        assert_eq!(config.download_dir, dirs::home_dir().unwrap().join("video"));
        assert_eq!(config.output_template, "%(id)s");
        assert_eq!(config.extra_args, ["--no-mtime"]);
        assert_eq!(config.aliases["m"], "get --audio");
        assert_eq!(
            config.env,
            BTreeMap::from([("A".into(), "1".into()), ("B".into(), String::new())])
        );
    }

    #[test]
    fn rejects_empty_settings() {
        assert!(load_error("yt_dlp_binary = ' '\n").ends_with("has an empty yt_dlp_binary"));
        assert!(load_error("ffmpeg_binary = ''\n").ends_with("has an empty ffmpeg_binary"));
        assert!(load_error("download_dir = ''\n").ends_with("has an empty download_dir"));
        assert!(load_error("output_template = ' '\n").ends_with("has an empty output_template"));
    }

    #[test]
    fn rejects_invalid_files() {
        assert!(load_error("download_dir = [\n").starts_with("parse config"));
        assert!(load_error("unknown = 1\n").starts_with("parse config"));
        assert!(load_error("[env]\nA = 1\n").starts_with("parse config"));
    }

    #[test]
    fn rejects_invalid_environment_variables() {
        let invalid = "invalid environment variable";
        assert!(load_error("[env]\n\"A=B\" = \"1\"\n").contains(invalid));
        assert!(load_error("[env]\n\"\" = \"1\"\n").contains(invalid));
        assert!(load_error("[env]\nA = \"\\u0000\"\n").contains(invalid));
    }

    #[test]
    fn expands_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_home("~".into()).unwrap(), home);
        assert_eq!(expand_home("~/t".into()).unwrap(), home.join("t"));
        assert_eq!(expand_home("/a/~".into()).unwrap(), PathBuf::from("/a/~"));
        assert_eq!(expand_home("~user".into()).unwrap(), PathBuf::from("~user"));
    }

    #[test]
    fn starter_config_is_all_comments_and_matches_defaults_when_uncommented() {
        assert!(STARTER.lines().all(|line| line.starts_with('#')));
        let uncommented: String = STARTER
            .lines()
            .skip(2)
            .map(|line| format!("{}\n", line.strip_prefix("# ").unwrap()))
            .collect();
        let (_dir, path) = write_config(&uncommented);
        let config = load(&path).unwrap();
        let expected = defaults();
        assert_eq!(config.yt_dlp_binary, expected.yt_dlp_binary);
        assert_eq!(config.ffmpeg_binary, expected.ffmpeg_binary);
        assert_eq!(config.download_dir, expected.download_dir);
        assert_eq!(config.output_template, expected.output_template);
        assert_eq!(config.extra_args, expected.extra_args);
        assert_eq!(config.aliases.len(), 1);
        assert_eq!(config.env.len(), 1);
    }
}
