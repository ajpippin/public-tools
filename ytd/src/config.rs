use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const STARTER: &str = "# ytd configuration. Uncomment and edit values to override defaults.\n\
# yt_dlp_binary: yt-dlp\n\
# download_dir: ~/Downloads\n\
# output_template: \"%(title)s [%(id)s].%(ext)s\"\n\
# extra_args: []\n";

#[derive(Clone, Debug)]
pub struct Config {
    pub yt_dlp_binary: String,
    pub download_dir: PathBuf,
    pub output_template: String,
    pub extra_args: Vec<String>,
}

pub fn default_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("YTD_CONFIG").filter(|value| !value.is_empty()) {
        return Ok(path.into());
    }
    Ok(dirs::home_dir()
        .context("cannot determine home directory")?
        .join(".config/tools/ytd.yaml"))
}

pub fn defaults() -> Config {
    Config {
        yt_dlp_binary: "yt-dlp".into(),
        download_dir: dirs::home_dir().unwrap_or_default().join("Downloads"),
        output_template: "%(title)s [%(id)s].%(ext)s".into(),
        extra_args: vec![],
    }
}

#[derive(Default, Deserialize)]
struct ConfigFile {
    yt_dlp_binary: Option<String>,
    download_dir: Option<PathBuf>,
    output_template: Option<String>,
    extra_args: Option<Vec<String>>,
}

pub fn load(path: &Path) -> Result<Config> {
    let mut settings = defaults();
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(settings),
        Err(error) => return Err(error).with_context(|| format!("read config {:?}", path)),
    };
    let file: ConfigFile = serde_yaml::from_str::<Option<ConfigFile>>(&text)
        .with_context(|| format!("parse config {:?}", path))?
        .unwrap_or_default();
    if let Some(v) = file.yt_dlp_binary {
        settings.yt_dlp_binary = v;
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
    if settings.yt_dlp_binary.trim().is_empty() {
        bail!("config {:?} has an empty yt_dlp_binary", path);
    }
    if settings.download_dir.as_os_str().is_empty() {
        bail!("config {:?} has an empty download_dir", path);
    }
    if settings.output_template.trim().is_empty() {
        bail!("config {:?} has an empty output_template", path);
    }
    settings.download_dir = expand_home(settings.download_dir)?;
    Ok(settings)
}

pub fn initialize(path: &Path) -> Result<bool> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => return Err(error).with_context(|| format!("create config {:?}", path)),
    };
    file.write_all(STARTER.as_bytes())?;
    Ok(true)
}

pub fn ensure(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    initialize(path)
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
        let path = dir.path().join("config.yaml");
        fs::write(&path, text).unwrap();
        (dir, path)
    }

    fn load_error(text: &str) -> String {
        let (_dir, path) = write_config(text);
        load(&path).unwrap_err().to_string()
    }

    #[test]
    fn missing_or_empty_config_uses_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = load(&dir.path().join("missing.yaml")).unwrap();
        assert_eq!(config.yt_dlp_binary, "yt-dlp");
        assert!(config.download_dir.ends_with("Downloads"));
        let (_dir, path) = write_config("");
        assert_eq!(
            load(&path).unwrap().output_template,
            defaults().output_template
        );
    }

    #[test]
    fn loads_and_expands_config() {
        let (_dir, path) = write_config(
            "yt_dlp_binary: /opt/yt-dlp\ndownload_dir: ~/video\noutput_template: '%(id)s'\nextra_args:\n  - --no-mtime\n",
        );
        let config = load(&path).unwrap();
        assert_eq!(config.yt_dlp_binary, "/opt/yt-dlp");
        assert_eq!(config.download_dir, dirs::home_dir().unwrap().join("video"));
        assert_eq!(config.output_template, "%(id)s");
        assert_eq!(config.extra_args, ["--no-mtime"]);
    }

    #[test]
    fn rejects_empty_settings() {
        assert!(load_error("yt_dlp_binary: ' '\n").ends_with("has an empty yt_dlp_binary"));
        assert!(load_error("download_dir: ''\n").ends_with("has an empty download_dir"));
        assert!(load_error("output_template: ' '\n").ends_with("has an empty output_template"));
        assert!(load_error("download_dir: [unterminated\n").starts_with("parse config"));
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
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        assert!(ensure(&path).unwrap());
        assert!(!ensure(&path).unwrap());
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.lines().all(|line| line.starts_with('#')));
        let uncommented: String = text
            .lines()
            .skip(1)
            .map(|line| format!("{}\n", line.strip_prefix("# ").unwrap()))
            .collect();
        fs::write(&path, uncommented).unwrap();
        let config = load(&path).unwrap();
        let expected = defaults();
        assert_eq!(config.yt_dlp_binary, expected.yt_dlp_binary);
        assert_eq!(config.download_dir, expected.download_dir);
        assert_eq!(config.output_template, expected.output_template);
        assert_eq!(config.extra_args, expected.extra_args);
    }
}
