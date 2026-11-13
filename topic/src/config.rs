use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
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
        .map(|base| base.join("tools/topic.yaml"))
        .collect();
    paths.extend(bases.iter().map(|base| base.join("topic.yaml")));
    let found = paths.iter().position(|path| path.exists()).unwrap_or(0);
    Ok(paths.swap_remove(found))
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
    fn path_lookup_prefers_tools_directories_and_xdg() {
        let dir = tempfile::tempdir().unwrap();
        let xdg = dir.path().join("xdg");
        let home = dir.path().join("home");
        let path = |xdg: Option<&Path>| {
            path_in(xdg.map(|xdg| xdg.as_os_str().into()), Some(home.clone())).unwrap()
        };
        assert_eq!(path(Some(&xdg)), xdg.join("tools/topic.yaml"));
        assert_eq!(path(None), home.join(".config/tools/topic.yaml"));
        for value in ["", "relative"] {
            assert_eq!(
                path_in(Some(value.into()), Some(home.clone())).unwrap(),
                home.join(".config/tools/topic.yaml")
            );
        }
        // Each file created outranks the ones before it.
        for expected in [
            home.join(".config/topic.yaml"),
            xdg.join("topic.yaml"),
            home.join(".config/tools/topic.yaml"),
            xdg.join("tools/topic.yaml"),
        ] {
            fs::create_dir_all(expected.parent().unwrap()).unwrap();
            fs::write(&expected, "").unwrap();
            assert_eq!(path(Some(&xdg)), expected);
        }
        assert_eq!(path(None), home.join(".config/tools/topic.yaml"));
        assert!(path_in(None, None).is_err());
        assert_eq!(
            path_in(Some(xdg.as_os_str().into()), None).unwrap(),
            xdg.join("tools/topic.yaml")
        );
    }
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
        assert!(config.topic_dir.ends_with(".topics"));
        assert_eq!(config.gpg_binary, "gpg");
        assert_eq!(config.aliases, default_aliases());
        let (_dir, path) = write_config("");
        assert_eq!(load(&path).unwrap().aliases, default_aliases());
    }

    #[test]
    fn configured_aliases_replace_defaults() {
        let (_dir, path) = write_config("aliases:\n  s: sections\n  list: list\n");
        assert_eq!(
            load(&path).unwrap().aliases,
            BTreeMap::from([
                ("list".into(), "list".into()),
                ("s".into(), "sections".into())
            ])
        );
    }

    #[test]
    fn rejects_invalid_aliases() {
        assert!(load_error("aliases:\n  'a b': list\n").contains("invalid alias \"a b\""));
        assert!(load_error("aliases:\n  ' ': list\n").contains("invalid alias"));
        assert!(
            load_error("aliases:\n  x: delete\n")
                .contains("maps alias \"x\" to unknown command \"delete\"")
        );
        assert!(
            load_error("aliases:\n  list: print\n")
                .contains("uses command name \"list\" as an alias")
        );
    }

    #[test]
    fn rejects_empty_settings() {
        assert!(load_error("topic_dir: ''\n").ends_with("has an empty topic_dir"));
        assert!(load_error("gpg_binary: ' '\n").ends_with("has an empty gpg_binary"));
        assert!(load_error("topic_dir: [unterminated\n").starts_with("parse config"));
    }

    #[test]
    fn expands_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_home("~".into()).unwrap(), home);
        assert_eq!(expand_home("~/t".into()).unwrap(), home.join("t"));
        assert_eq!(expand_home("/a/~".into()).unwrap(), PathBuf::from("/a/~"));
        assert_eq!(expand_home("~user".into()).unwrap(), PathBuf::from("~user"));
        let (_dir, path) = write_config("topic_dir: ~/notes\n");
        assert_eq!(load(&path).unwrap().topic_dir, home.join("notes"));
    }

    #[test]
    fn starter_config_is_all_comments_and_loads_when_uncommented() {
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
        assert_eq!(config.aliases, default_aliases());
        assert_eq!(config.gpg_recipients.len(), 1);
    }
}
