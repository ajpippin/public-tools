use anyhow::{Context, Result, bail};
use chrono::Local;
use regex::RegexBuilder;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct Store {
    pub dir: PathBuf,
    pub gpg_binary: String,
}
pub struct SearchResults {
    pub lines: Vec<String>,
    pub files: Vec<PathBuf>,
}

impl Store {
    pub fn files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.dir)
            .with_context(|| format!("read topic directory {:?}", self.dir))?
        {
            let entry = entry?;
            if entry.file_type()?.is_file() && topic_stem(&entry.path()).is_some() {
                files.push(entry.path());
            }
        }
        files.sort();
        Ok(files)
    }

    pub fn find(&self, value: &str) -> Result<PathBuf> {
        let input = Path::new(value)
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or(value);
        let base = topic_stem(Path::new(input)).unwrap_or_else(|| {
            Path::new(input)
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or(input)
                .to_owned()
        });
        let number: u16 = base
            .parse()
            .map_err(|_| anyhow::anyhow!("{value}: no topic found"))?;
        if number > 9999 {
            bail!("{value}: no topic found");
        }
        let stem = format!("{number:04}");
        let found: Vec<_> = [".txt", ".md", ".txt.gpg", ".md.gpg", ".gpg"]
            .iter()
            .map(|ext| self.dir.join(format!("{stem}{ext}")))
            .filter(|path| path.is_file())
            .collect();
        match found.as_slice() {
            [path] => Ok(path.clone()),
            [] => bail!("{value}: no topic found"),
            _ => bail!(
                "{value}: multiple topic files found: {}",
                found
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    pub fn list(&self, section: Option<&str>, sections_only: bool) -> Result<Vec<String>> {
        let mut results = Vec::new();
        for path in self.files()? {
            let stem = topic_stem(&path).expect("validated topic");
            if sections_only && !stem.ends_with("00") {
                continue;
            }
            if let Some(section) = section {
                let prefix = if matches!(section, "0" | "00") {
                    "0"
                } else {
                    section
                };
                if !stem.starts_with(prefix) {
                    continue;
                }
            }
            for line in self.read_lines(&path)? {
                if !line.contains("@topic") || (sections_only && !line.contains("~~~")) {
                    continue;
                }
                let text = line.replacen("@topic", "", 1);
                if !text.trim().is_empty() {
                    results.push(format!("{stem}:{text}"));
                }
            }
        }
        Ok(results)
    }

    pub fn search(&self, terms: &[String], full: bool) -> Result<SearchResults> {
        let expression = if terms.is_empty() {
            ".*".into()
        } else {
            format!("({})", terms.join("|"))
        };
        let regex = RegexBuilder::new(&expression)
            .case_insensitive(true)
            .build()
            .context("invalid search expression")?;
        let mut lines = Vec::new();
        let mut files = BTreeSet::new();
        for path in self.files()? {
            for line in self.read_lines(&path)? {
                if (!full && !line.contains("@topic") && !line.contains("@search"))
                    || !regex.is_match(&line)
                {
                    continue;
                }
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if full {
                    if !line.trim().is_empty() {
                        lines.push(format!("{name}: {line}"));
                        files.insert(path.clone());
                    }
                } else {
                    let text = line.replacen("@topic", "", 1).replacen("@search", "", 1);
                    if !text.trim().is_empty() {
                        lines.push(format!("{}:{text}", topic_stem(&path).unwrap()));
                        files.insert(path.clone());
                    }
                }
            }
        }
        lines.sort();
        Ok(SearchResults {
            lines,
            files: files.into_iter().collect(),
        })
    }

    pub fn read_topic(&self, path: &Path) -> Result<String> {
        let mut content = String::from_utf8(self.read_data(path)?).context("topic is not UTF-8")?;
        if !content.ends_with('\n') {
            content.push('\n');
        }
        Ok(format!(
            "\n-- {} {}\n{}{}\n\n",
            path.file_name().unwrap_or_default().to_string_lossy(),
            "-".repeat(72),
            content,
            "-".repeat(80)
        ))
    }

    pub fn read_data(&self, path: &Path) -> Result<Vec<u8>> {
        if !is_encrypted(path) {
            return Ok(fs::read(path)?);
        }
        require_gpg(&self.gpg_binary)?;
        let output = Command::new(&self.gpg_binary)
            .args(["--decrypt"])
            .arg(path)
            .output()?;
        if !output.status.success() {
            bail!(
                "decrypt {}: {}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(output.stdout)
    }

    pub fn encrypt(&self, plaintext: &Path, encrypted: &Path, recipients: &[String]) -> Result<()> {
        require_gpg(&self.gpg_binary)?;
        let recipients: Vec<_> = recipients.iter().filter(|v| !v.trim().is_empty()).collect();
        if recipients.is_empty() {
            bail!("gpg_recipients contains no usable recipients");
        }
        let mut args = vec!["--batch", "--yes", "--output", "-", "--encrypt"];
        for recipient in &recipients {
            args.extend(["--recipient", recipient.as_str()]);
        }
        let output = Command::new(&self.gpg_binary)
            .args(args)
            .arg(plaintext)
            .output()?;
        if !output.status.success() {
            bail!(
                "encrypt {}: {}",
                encrypted.file_name().unwrap_or_default().to_string_lossy(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let parent = encrypted.parent().unwrap_or_else(|| Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp.as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        temp.write_all(&output.stdout)?;
        temp.persist(encrypted).map_err(|error| error.error)?;
        Ok(())
    }

    fn read_lines(&self, path: &Path) -> Result<Vec<String>> {
        Ok(String::from_utf8(self.read_data(path)?)
            .context("topic is not UTF-8")?
            .lines()
            .map(str::to_owned)
            .collect())
    }
}

pub fn topic_stem(path: &Path) -> Option<String> {
    let mut name = path.file_name()?.to_str()?.to_owned();
    let encrypted = name.to_lowercase().ends_with(".gpg");
    if encrypted {
        name.truncate(name.len() - 4);
    }
    let lower = name.to_lowercase();
    if lower.ends_with(".txt") {
        name.truncate(name.len() - 4);
    } else if lower.ends_with(".md") {
        name.truncate(name.len() - 3);
    } else if !encrypted {
        return None;
    }
    (name.len() == 4 && name.chars().all(|c| c.is_ascii_digit())).then_some(name)
}

pub fn is_encrypted(path: &Path) -> bool {
    path.extension()
        .is_some_and(|v| v.eq_ignore_ascii_case("gpg"))
}

pub fn require_gpg(binary: &str) -> Result<()> {
    let found = if Path::new(binary).components().count() > 1 {
        Path::new(binary).is_file()
    } else {
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join(binary).is_file())
        })
    };
    if !found {
        bail!(
            "GPG executable {binary:?} was not found; please install GnuPG or configure gpg_binary with its location"
        );
    }
    Ok(())
}

pub fn validate_author(name: &str, email: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("author_name is required in the topic configuration");
    }
    if email.trim().is_empty() {
        bail!("author_email is required in the topic configuration");
    }
    Ok(())
}

pub fn update_footer(path: &Path, name: &str, email: &str) -> Result<()> {
    validate_author(name, email)?;
    let metadata = fs::metadata(path)?;
    let content = fs::read_to_string(path)?.replace("\r\n", "\n");
    let mut lines: Vec<String> = content.split('\n').map(str::to_owned).collect();
    if let Some(updated) = lines
        .iter()
        .rposition(|line| line.contains("Last updated:"))
    {
        let mut start = updated;
        let mut previous = updated;
        while previous > 0 && lines[previous - 1].trim().is_empty() {
            previous -= 1;
        }
        if previous > 0 && is_horizontal_rule(&lines[previous - 1]) {
            start = previous - 1;
        }
        let mut end = updated + 1;
        while end < lines.len() && lines[end].trim().is_empty() {
            end += 1;
        }
        lines.drain(start..end);
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines.extend([
        String::new(),
        "---".into(),
        format!(
            "Last updated: {}, by \"{} <{}>\"",
            Local::now().format("%Y-%m-%d"),
            name.trim(),
            email.trim()
        ),
    ]);
    fs::write(path, format!("{}\n", lines.join("\n")))?;
    fs::set_permissions(path, metadata.permissions())?;
    Ok(())
}

fn is_horizontal_rule(line: &str) -> bool {
    let text = line.trim();
    text.len() >= 3 && text.chars().all(|c| c == '-')
}

pub fn run_interactive(dir: &Path, program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if !status.success() {
        bail!("{program} {}: exited with {status}", args.join(" "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        for (name, text) in [
            (
                "0001.txt",
                "@topic First topic\n@search alpha beta\nBody needle\n",
            ),
            ("0100.md", "@topic ~~~ First section\n"),
            ("0101.md", "@topic Markdown topic\n@search gamma\n"),
            ("notes.txt", "@topic ignored\n"),
        ] {
            fs::write(dir.path().join(name), text).unwrap();
        }
        let store = Store {
            dir: dir.path().into(),
            gpg_binary: "gpg".into(),
        };
        (dir, store)
    }
    #[test]
    fn finds_and_pads() {
        let (_d, s) = fixture();
        assert_eq!(s.find("1").unwrap().file_name().unwrap(), "0001.txt");
    }
    #[test]
    fn accepts_encrypted_names() {
        for name in ["0001.gpg", "0002.txt.gpg", "0003.md.gpg"] {
            assert!(topic_stem(Path::new(name)).is_some());
        }
        for name in ["topic.gpg", "0001.asc", "00001.gpg"] {
            assert!(topic_stem(Path::new(name)).is_none());
        }
    }
    #[test]
    fn lists_sections() {
        let (_d, s) = fixture();
        assert_eq!(s.list(None, true).unwrap(), ["0100: ~~~ First section"]);
    }
    #[test]
    fn searches_metadata_and_content() {
        let (_d, s) = fixture();
        assert_eq!(
            s.search(&["alpha".into()], false).unwrap().lines,
            ["0001: alpha beta"]
        );
        assert_eq!(
            s.search(&["needle".into()], true).unwrap().lines,
            ["0001.txt: Body needle"]
        );
    }
    #[test]
    fn footer_requires_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0001.txt");
        fs::write(&path, "body\n").unwrap();
        assert!(update_footer(&path, "", "x@y").is_err());
    }
}
