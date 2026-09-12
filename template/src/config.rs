use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    pub template_dirs: Vec<PathBuf>,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_email: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub substitutions: HashMap<String, String>,
}

#[derive(Default, Deserialize)]
struct ConfigFile {
    template_dirs: Option<Vec<PathBuf>>,
    author_name: Option<String>,
    author_email: Option<String>,
    substitutions: Option<HashMap<String, String>>,
}

pub fn default_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("TEMPLATE_CONFIG").filter(|value| !value.is_empty()) {
        return Ok(path.into());
    }
    Ok(dirs::home_dir()
        .context("cannot determine the home directory")?
        .join(".config/tools/template.yaml"))
}

pub fn defaults() -> Config {
    let home = dirs::home_dir().unwrap_or_default();
    let mut template_dirs = vec![
        PathBuf::from("."),
        PathBuf::from(".templates.d"),
        home.join(".local/etc/templates.d"),
        PathBuf::from("/etc/templates"),
    ];
    if let Some(extra) = env::var_os("TEMPLATE_DIR").filter(|value| !value.is_empty()) {
        template_dirs = std::iter::once(PathBuf::from("."))
            .chain(env::split_paths(&extra))
            .chain(template_dirs.into_iter().skip(1))
            .collect();
    }
    Config {
        template_dirs: unique_paths(template_dirs),
        author_name: git_config("user.name")
            .or_else(user_name)
            .unwrap_or_default(),
        author_email: git_config("user.email").unwrap_or_else(|| {
            let user = env::var("USER")
                .or_else(|_| env::var("USERNAME"))
                .unwrap_or_default();
            let host = hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if user.is_empty() {
                String::new()
            } else {
                format!("{user}@{host}")
            }
        }),
        substitutions: HashMap::new(),
    }
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
    if let Some(value) = file.template_dirs {
        settings.template_dirs = value;
    }
    if let Some(value) = file.author_name {
        settings.author_name = value;
    }
    if let Some(value) = file.author_email {
        settings.author_email = value;
    }
    if let Some(value) = file.substitutions {
        settings.substitutions = value;
    }
    if settings.template_dirs.is_empty() {
        bail!("config {:?} has no template_dirs", path);
    }
    settings.template_dirs = unique_paths(
        settings
            .template_dirs
            .into_iter()
            .map(expand_home)
            .collect::<Result<Vec<_>>>()?,
    );
    Ok(settings)
}

pub fn initialize(path: &Path) -> Result<bool> {
    let settings = defaults();
    let yaml = format!(
        "# Template configuration. Uncomment and edit values to override defaults.\n\
# template_dirs:\n\
#   - .\n\
#   - .templates.d\n\
#   - ~/.local/etc/templates.d\n\
#   - /etc/templates\n\
# author_name: {}\n\
# author_email: {}\n\
# substitutions:\n\
#   copyright: Copyright (c)\n",
        settings.author_name, settings.author_email
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => return Err(error).with_context(|| format!("create config {:?}", path)),
    };
    file.write_all(yaml.as_bytes())?;
    Ok(true)
}

pub fn ensure(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    initialize(path)
}

fn expand_home(path: PathBuf) -> Result<PathBuf> {
    let text = path.to_string_lossy();
    if text == "~" {
        return dirs::home_dir().context("cannot determine home directory");
    }
    if let Some(rest) = text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\")) {
        return Ok(dirs::home_dir()
            .context("cannot determine home directory")?
            .join(rest));
    }
    Ok(path)
}

fn unique_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| !path.as_os_str().is_empty() && seen.insert(path.clone()))
        .collect()
}

fn git_config(key: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", key])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn user_name() -> Option<String> {
    env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "template_dirs:\n  - /tmp/templates\nauthor_name: Test User\nauthor_email: test@example.com\nsubstitutions:\n  revision: '3.0'\n").unwrap();
        let config = load(&path).unwrap();
        assert_eq!(config.template_dirs, [PathBuf::from("/tmp/templates")]);
        assert_eq!(config.substitutions["revision"], "3.0");
    }

    #[test]
    fn initialize_does_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.yaml");
        assert!(initialize(&path).unwrap());
        assert!(!initialize(&path).unwrap());
    }

    #[test]
    fn omitted_fields_keep_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "author_name: Test User\n").unwrap();
        let config = load(&path).unwrap();
        assert_eq!(config.author_name, "Test User");
        assert!(!config.template_dirs.is_empty());
    }
}
