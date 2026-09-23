use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Model {
    pub name: String,
    pub slot: String,
    #[serde(default)]
    pub purpose: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HostShelf {
    pub hostname: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ollama_host_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub models_path: String,
    pub shelves: BTreeMap<String, Vec<Model>>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Config {
    #[serde(default)]
    hosts: Vec<HostShelf>,
}

pub fn configured_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("OLLAMA_SHELF_CONFIG").filter(|p| !p.is_empty()) {
        return Ok(path.into());
    }
    let home = dirs::home_dir().context("cannot determine the home directory")?;
    Ok(home.join(".config/tools/shelves.yaml"))
}

pub fn initialize(path: &Path, hostname: &str) -> Result<()> {
    if hostname.trim().is_empty() {
        bail!("hostname cannot be empty");
    }
    let yaml = commented_config(hostname);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create config directory {:?}", parent))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                anyhow::anyhow!("config {:?} already exists", path)
            } else {
                anyhow::anyhow!(error).context(format!("create config {:?}", path))
            }
        })?;
    file.write_all(yaml.as_bytes())
        .with_context(|| format!("write config {:?}", path))?;
    Ok(())
}

pub fn load(path: &Path, hostname: &str) -> Result<HostShelf> {
    let text = fs::read_to_string(path).with_context(|| format!("read config {:?}", path))?;
    let config = serde_yaml::from_str::<Option<Config>>(&text)
        .with_context(|| format!("parse config {:?}", path))?
        .unwrap_or_default();
    if config.hosts.is_empty() {
        let shelf = initial_config(hostname)
            .hosts
            .into_iter()
            .next()
            .expect("initial config has one host");
        return Ok(with_defaults(shelf));
    }
    let short = hostname.split('.').next().unwrap_or(hostname);
    let selected = config
        .hosts
        .iter()
        .find(|host| host.hostname == hostname)
        .or_else(|| {
            (short != hostname)
                .then(|| config.hosts.iter().find(|host| host.hostname == short))
                .flatten()
        })
        .or_else(|| {
            config
                .hosts
                .iter()
                .rev()
                .find(|host| host.hostname == "default" || host.hostname == "*")
        })
        .cloned()
        .with_context(|| {
            format!("no shelf configured for hostname {hostname:?} (and no default shelf)")
        })?;
    validate(&selected)?;
    Ok(with_defaults(selected))
}

pub fn ensure(path: &Path, hostname: &str) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    initialize(path, hostname)?;
    Ok(true)
}

fn commented_config(hostname: &str) -> String {
    format!(
        "# Ollama shelf configuration. Uncomment and edit values to override defaults.\n\
# hosts:\n\
#   - hostname: {hostname}\n\
#     ollama_host_url: http://127.0.0.1:11434\n\
#     models_path: ~/.ollama/models\n\
#     shelves:\n\
#       core:\n\
#         - name: llama3.2:3b\n\
#           slot: tiny\n\
#           purpose: fast chat, routing, and first-pass classification\n\
#         - name: llama3.1:8b\n\
#           slot: workhorse\n\
#           purpose: general chat, tool calling, and RAG generation\n\
#         - name: qwen2.5-coder:7b\n\
#           slot: coder\n\
#           purpose: code generation and FIM\n\
#         - name: nomic-embed-text\n\
#           slot: embedder\n\
#           purpose: embeddings for search and RAG\n\
#       optional:\n\
#         - name: llama3.2:1b\n\
#           slot: tiny\n\
#           purpose: autocomplete and ultra-cheap routing\n\
#         - name: qwen2.5-coder:14b\n\
#           slot: coder+\n\
#           purpose: harder refactors when speed is not critical\n\
#         - name: deepseek-r1:8b\n\
#           slot: reasoner\n\
#           purpose: multi-step reasoning\n\
#         - name: mxbai-embed-large\n\
#           slot: embedder+\n\
#           purpose: higher-quality embeddings\n"
    )
}

impl HostShelf {
    pub fn models(&self, name: &str) -> Result<Vec<Model>> {
        if name == "all" {
            return Ok(self.all_models());
        }
        self.shelves
            .get(name)
            .cloned()
            .with_context(|| format!("shelf {name:?} is not configured"))
    }

    pub fn all_models(&self) -> Vec<Model> {
        let mut seen = HashSet::new();
        self.shelves
            .values()
            .flatten()
            .filter(|model| seen.insert(model.name.clone()))
            .cloned()
            .collect()
    }

    pub fn benchmark_targets(&self, name: &str) -> Result<Vec<String>> {
        if name == "all" {
            return Ok(self
                .all_models()
                .into_iter()
                .map(|model| model.name)
                .collect());
        }
        let shelf = self.shelves.get(name);
        let model_exists = self.all_models().iter().any(|model| model.name == name);
        if shelf.is_some() && model_exists {
            bail!(
                "{name:?} is both a shelf name and a model name; rename one to remove the conflict"
            );
        }
        Ok(match shelf {
            Some(models) => models.iter().map(|model| model.name.clone()).collect(),
            None => vec![name.to_owned()],
        })
    }
}

fn validate(shelf: &HostShelf) -> Result<()> {
    if shelf.shelves.is_empty() {
        bail!("host has no shelves");
    }
    shelf
        .shelves
        .iter()
        .try_for_each(|(name, models)| validate_shelf(name, models))
}

fn validate_shelf(name: &str, models: &[Model]) -> Result<()> {
    if name.trim().is_empty() {
        bail!("shelf names cannot be empty");
    }
    if name == "all" {
        bail!("shelf name \"all\" is reserved for all models in all shelves");
    }
    let mut seen = HashSet::new();
    for model in models {
        validate_model(name, model)?;
        if !seen.insert(&model.name) {
            bail!(
                "model {:?} is listed more than once in shelf {name:?}",
                model.name
            );
        }
    }
    Ok(())
}

fn validate_model(shelf: &str, model: &Model) -> Result<()> {
    if model.name.trim().is_empty() || model.slot.trim().is_empty() {
        bail!("every entry in shelf {shelf:?} must have a name and slot");
    }
    if model.name == "all" {
        bail!("model name \"all\" in shelf {shelf:?} is reserved for all models in all shelves");
    }
    Ok(())
}

fn with_defaults(mut shelf: HostShelf) -> HostShelf {
    if shelf.ollama_host_url.is_empty() {
        shelf.ollama_host_url = "http://127.0.0.1:11434".into();
    }
    if let Some(value) = env::var_os("OLLAMA_HOST_URL").filter(|value| !value.is_empty()) {
        shelf.ollama_host_url = value.to_string_lossy().into_owned();
    }
    if shelf.models_path.is_empty() {
        shelf.models_path = env::var_os("OLLAMA_MODELS")
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string_lossy().into_owned())
            .or_else(|| {
                dirs::home_dir()
                    .map(|home| home.join(".ollama/models").to_string_lossy().into_owned())
            })
            .unwrap_or_default();
    }
    shelf
}

fn initial_config(hostname: &str) -> Config {
    let model = |name: &str, slot: &str, purpose: &str| Model {
        name: name.into(),
        slot: slot.into(),
        purpose: purpose.into(),
    };
    Config {
        hosts: vec![HostShelf {
            hostname: hostname.into(),
            ollama_host_url: "http://127.0.0.1:11434".into(),
            models_path: String::new(),
            shelves: BTreeMap::from([
                (
                    "core".into(),
                    vec![
                        model(
                            "llama3.2:3b",
                            "tiny",
                            "fast chat, routing, and first-pass classification",
                        ),
                        model(
                            "llama3.1:8b",
                            "workhorse",
                            "general chat, tool calling, and RAG generation",
                        ),
                        model("qwen2.5-coder:7b", "coder", "code generation and FIM"),
                        model(
                            "nomic-embed-text",
                            "embedder",
                            "embeddings for search and RAG",
                        ),
                    ],
                ),
                (
                    "optional".into(),
                    vec![
                        model(
                            "llama3.2:1b",
                            "tiny",
                            "autocomplete and ultra-cheap routing",
                        ),
                        model(
                            "qwen2.5-coder:14b",
                            "coder+",
                            "harder refactors when speed is not critical",
                        ),
                        model("deepseek-r1:8b", "reasoner", "multi-step reasoning"),
                        model(
                            "mxbai-embed-large",
                            "embedder+",
                            "higher-quality embeddings",
                        ),
                    ],
                ),
            ]),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_exact_short_and_fallback_hosts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shelves.yaml");
        fs::write(
            &path,
            r#"hosts:
  - hostname: default
    shelves: {core: [{name: fallback, slot: workhorse}]}
  - hostname: devbox
    shelves: {core: [{name: short, slot: coder}]}
  - hostname: devbox.example.com
    shelves: {core: [{name: exact, slot: coder}]}
"#,
        )
        .unwrap();
        assert_eq!(
            load(&path, "devbox.example.com").unwrap().shelves["core"][0].name,
            "exact"
        );
        assert_eq!(
            load(&path, "devbox.other").unwrap().shelves["core"][0].name,
            "short"
        );
        assert_eq!(
            load(&path, "unknown").unwrap().shelves["core"][0].name,
            "fallback"
        );
    }

    #[test]
    fn all_targets_are_deduplicated() {
        let shelf = HostShelf {
            hostname: "x".into(),
            ollama_host_url: String::new(),
            models_path: String::new(),
            shelves: BTreeMap::from([
                (
                    "core".into(),
                    vec![Model {
                        name: "shared".into(),
                        slot: "workhorse".into(),
                        purpose: String::new(),
                    }],
                ),
                (
                    "extra".into(),
                    vec![Model {
                        name: "shared".into(),
                        slot: "workhorse".into(),
                        purpose: String::new(),
                    }],
                ),
            ]),
        };
        assert_eq!(shelf.benchmark_targets("all").unwrap(), vec!["shared"]);
    }

    #[test]
    fn validate_rejects_invalid_shelves() {
        let model = |name: &str, slot: &str| Model {
            name: name.into(),
            slot: slot.into(),
            purpose: String::new(),
        };
        let shelf = |shelves: Vec<(&str, Vec<Model>)>| HostShelf {
            hostname: "x".into(),
            ollama_host_url: String::new(),
            models_path: String::new(),
            shelves: shelves
                .into_iter()
                .map(|(name, models)| (name.into(), models))
                .collect(),
        };
        let error = |host: HostShelf| validate(&host).unwrap_err().to_string();

        assert!(validate(&shelf(vec![("core", vec![model("a", "tiny")])])).is_ok());
        assert_eq!(error(shelf(vec![])), "host has no shelves");
        assert_eq!(
            error(shelf(vec![(" ", vec![])])),
            "shelf names cannot be empty"
        );
        assert!(error(shelf(vec![("all", vec![])])).contains("reserved"));
        assert!(error(shelf(vec![("core", vec![model("a", "")])])).contains("name and slot"));
        assert!(error(shelf(vec![("core", vec![model("all", "tiny")])])).contains("reserved"));
        assert!(
            error(shelf(vec![(
                "core",
                vec![model("a", "tiny"), model("a", "coder")]
            )]))
            .contains("more than once")
        );
    }

    fn write_config(text: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shelves.yaml");
        fs::write(&path, text).unwrap();
        (dir, path)
    }

    fn two_shelves() -> HostShelf {
        let model = |name: &str| Model {
            name: name.into(),
            slot: "tiny".into(),
            purpose: String::new(),
        };
        HostShelf {
            hostname: "x".into(),
            ollama_host_url: String::new(),
            models_path: String::new(),
            shelves: BTreeMap::from([
                ("core".into(), vec![model("a"), model("b")]),
                ("extra".into(), vec![model("b"), model("c")]),
            ]),
        }
    }

    #[test]
    fn starter_comments_match_initial_config() {
        let uncommented: String = commented_config("devbox")
            .lines()
            .skip(1)
            .map(|line| format!("{}\n", line.strip_prefix("# ").unwrap()))
            .collect();
        let parsed: Config = serde_yaml::from_str(&uncommented).unwrap();
        let expected = initial_config("devbox");
        assert_eq!(parsed.hosts.len(), 1);
        assert_eq!(parsed.hosts[0].hostname, "devbox");
        assert_eq!(
            parsed.hosts[0].ollama_host_url,
            expected.hosts[0].ollama_host_url
        );
        assert_eq!(parsed.hosts[0].shelves, expected.hosts[0].shelves);
    }

    #[test]
    fn empty_or_commented_config_uses_builtin_shelf() {
        for text in ["", "hosts: []\n", &commented_config("devbox")] {
            let (_dir, path) = write_config(text);
            let shelf = load(&path, "devbox").unwrap();
            assert_eq!(shelf.hostname, "devbox");
            assert_eq!(
                shelf.shelves.keys().collect::<Vec<_>>(),
                ["core", "optional"]
            );
        }
    }

    #[test]
    fn wildcard_host_is_a_fallback() {
        let (_dir, path) = write_config(
            "hosts:\n  - hostname: '*'\n    shelves: {core: [{name: star, slot: tiny}]}\n",
        );
        assert_eq!(
            load(&path, "anything").unwrap().shelves["core"][0].name,
            "star"
        );
    }

    #[test]
    fn last_fallback_host_wins() {
        let (_dir, path) = write_config(
            "hosts:
  - hostname: default
    shelves: {core: [{name: first, slot: tiny}]}
  - hostname: '*'
    shelves: {core: [{name: last, slot: tiny}]}
",
        );
        assert_eq!(
            load(&path, "anything").unwrap().shelves["core"][0].name,
            "last"
        );
    }

    #[test]
    fn unknown_host_without_fallback_fails() {
        let (_dir, path) = write_config(
            "hosts:\n  - hostname: devbox\n    shelves: {core: [{name: a, slot: tiny}]}\n",
        );
        let error = load(&path, "laptop").unwrap_err().to_string();
        assert!(error.contains("no shelf configured for hostname \"laptop\""));
    }

    #[test]
    fn load_validates_selected_host() {
        let (_dir, path) = write_config(
            "hosts:\n  - hostname: devbox\n    shelves: {all: [{name: a, slot: tiny}]}\n",
        );
        assert!(
            load(&path, "devbox")
                .unwrap_err()
                .to_string()
                .contains("reserved")
        );
    }

    #[test]
    fn load_reports_parse_and_read_errors() {
        let (dir, path) = write_config("hosts: [unterminated\n");
        assert!(
            load(&path, "x")
                .unwrap_err()
                .to_string()
                .starts_with("parse config")
        );
        let missing = dir.path().join("missing.yaml");
        assert!(
            load(&missing, "x")
                .unwrap_err()
                .to_string()
                .starts_with("read config")
        );
    }

    #[test]
    fn explicit_settings_are_kept() {
        let (_dir, path) = write_config(
            "hosts:
  - hostname: devbox
    models_path: /srv/models
    shelves: {core: [{name: a, slot: tiny}]}
",
        );
        assert_eq!(load(&path, "devbox").unwrap().models_path, "/srv/models");
    }

    #[test]
    fn initialize_rejects_blank_hostname() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shelves.yaml");
        assert_eq!(
            initialize(&path, "  ").unwrap_err().to_string(),
            "hostname cannot be empty"
        );
        assert!(!path.exists());
    }

    #[test]
    fn ensure_creates_only_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/shelves.yaml");
        assert!(ensure(&path, "devbox").unwrap());
        fs::write(&path, "hosts: []\n").unwrap();
        assert!(!ensure(&path, "devbox").unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "hosts: []\n");
    }

    #[test]
    fn models_selects_one_shelf_or_all() {
        let shelf = two_shelves();
        let names = |models: Vec<Model>| models.into_iter().map(|m| m.name).collect::<Vec<_>>();
        assert_eq!(names(shelf.models("extra").unwrap()), ["b", "c"]);
        assert_eq!(names(shelf.models("all").unwrap()), ["a", "b", "c"]);
        assert_eq!(
            shelf.models("nope").unwrap_err().to_string(),
            "shelf \"nope\" is not configured"
        );
    }

    #[test]
    fn benchmark_targets_resolve_shelves_and_models() {
        let mut shelf = two_shelves();
        assert_eq!(shelf.benchmark_targets("core").unwrap(), ["a", "b"]);
        assert_eq!(shelf.benchmark_targets("a").unwrap(), ["a"]);
        // Unknown names are passed through as ad-hoc model names.
        assert_eq!(
            shelf.benchmark_targets("mistral:7b").unwrap(),
            ["mistral:7b"]
        );
        shelf.shelves.get_mut("extra").unwrap()[0].name = "core".into();
        assert!(
            shelf
                .benchmark_targets("core")
                .unwrap_err()
                .to_string()
                .contains("both a shelf name and a model name")
        );
    }
}
