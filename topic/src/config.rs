use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub topic_dir: PathBuf,
    #[serde(default)]
    pub author_name: String,
    #[serde(default)]
    pub author_email: String,
    #[serde(default = "default_gpg")]
    pub gpg_binary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpg_recipients: Vec<String>,
    #[serde(
        default = "default_aliases",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub aliases: BTreeMap<String, String>,
}

fn default_gpg() -> String {
    "gpg".into()
}
pub fn default_aliases() -> BTreeMap<String, String> {
    BTreeMap::from([("p".into(), "print".into()), ("q".into(), "search".into())])
}

pub fn default_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("TOPIC_CONFIG").filter(|value| !value.is_empty()) {
        return Ok(path.into());
    }
    Ok(dirs::home_dir()
        .context("cannot determine home directory")?
        .join(".config/tools/topic.yaml"))
}

pub fn defaults() -> Config {
    let user = env::var("USER")
        .or_else(|_| env::var("USERNAME"))
        .unwrap_or_default();
    let author_name = git_config("user.name").unwrap_or_else(|| user.clone());
    let author_email = git_config("user.email").unwrap_or_else(|| {
        let host = hostname::get()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if user.is_empty() {
            String::new()
        } else {
            format!("{user}@{host}")
        }
    });
    Config {
        topic_dir: dirs::home_dir().unwrap_or_default().join(".topics"),
        author_name,
        gpg_recipients: if author_email.is_empty() {
            vec![]
        } else {
            vec![author_email.clone()]
        },
        author_email,
        gpg_binary: default_gpg(),
        aliases: default_aliases(),
    }
}

#[derive(Default, Deserialize)]
struct ConfigFile {
    topic_dir: Option<PathBuf>,
    author_name: Option<String>,
    author_email: Option<String>,
    gpg_binary: Option<String>,
    gpg_recipients: Option<Vec<String>>,
    aliases: Option<BTreeMap<String, String>>,
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
    if let Some(v) = file.topic_dir {
        settings.topic_dir = v;
    }
    if let Some(v) = file.author_name {
        settings.author_name = v;
    }
    if let Some(v) = file.author_email {
        settings.author_email = v;
    }
    if let Some(v) = file.gpg_binary {
        settings.gpg_binary = v;
    }
    if let Some(v) = file.gpg_recipients {
        settings.gpg_recipients = v;
    }
    if let Some(v) = file.aliases {
        settings.aliases = v;
    }
    if settings.topic_dir.as_os_str().is_empty() {
        bail!("config {:?} has an empty topic_dir", path);
    }
    if settings.gpg_binary.trim().is_empty() {
        bail!("config {:?} has an empty gpg_binary", path);
    }
    validate_aliases(path, &settings.aliases)?;
    settings.topic_dir = expand_home(settings.topic_dir)?;
    Ok(settings)
}

pub fn initialize(path: &Path) -> Result<bool> {
    let settings = defaults();
    let recipients = if settings.author_email.is_empty() {
        "user@example.com"
    } else {
        &settings.author_email
    };
    let yaml = format!(
        "# Topic configuration. Uncomment and edit values to override defaults.\n\
# topic_dir: ~/.topics\n\
# author_name: {}\n\
# author_email: {}\n\
# gpg_binary: gpg\n\
# gpg_recipients:\n\
#   - {}\n\
# aliases:\n\
#   q: search\n\
#   p: print\n",
        settings.author_name, settings.author_email, recipients
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

pub fn expand_home(path: PathBuf) -> Result<PathBuf> {
    let text = path.to_string_lossy();
    if text == "~" {
        return dirs::home_dir().context("expand topic_dir: cannot determine home directory");
    }
    if let Some(rest) = text.strip_prefix("~/").or_else(|| text.strip_prefix("~\\")) {
        return Ok(dirs::home_dir()
            .context("expand topic_dir: cannot determine home directory")?
            .join(rest));
    }
    Ok(path)
}

fn validate_aliases(path: &Path, aliases: &BTreeMap<String, String>) -> Result<()> {
    let commands = [
        "edit", "init", "list", "print", "search", "sections", "version",
    ];
    for (alias, command) in aliases {
        if alias.trim().is_empty() || alias.chars().any(char::is_whitespace) {
            bail!("config {:?} has invalid alias {alias:?}", path);
        }
        if !commands.contains(&command.as_str()) {
            bail!(
                "config {:?} maps alias {alias:?} to unknown command {command:?}",
                path
            );
        }
        if commands.contains(&alias.as_str()) && alias != command {
            bail!("config {:?} uses command name {alias:?} as an alias", path);
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initialize_does_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.yaml");
        assert!(initialize(&path).unwrap());
        assert!(!initialize(&path).unwrap());
    }
    #[test]
    fn loads_and_expands_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        fs::write(&path, "topic_dir: /tmp/my-topics\nauthor_name: Test User\nauthor_email: test@example.com\ngpg_binary: /opt/gpg/bin/gpg\n").unwrap();
        let config = load(&path).unwrap();
        assert_eq!(config.topic_dir, PathBuf::from("/tmp/my-topics"));
        assert_eq!(config.gpg_binary, "/opt/gpg/bin/gpg");
    }
}
