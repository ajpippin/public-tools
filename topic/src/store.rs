use anyhow::{Context, Result, bail};
use chrono::Local;
use regex::{Regex, RegexBuilder};
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
        let prefix = section.map(section_prefix);
        let mut results = Vec::new();
        for path in self.files()? {
            let stem = topic_stem(&path).expect("validated topic");
            if !list_includes(&stem, prefix, sections_only) {
                continue;
            }
            let lines = self.read_lines(&path)?;
            results.extend(
                lines
                    .iter()
                    .filter_map(|line| list_entry(line, sections_only))
                    .map(|text| format!("{stem}:{text}")),
            );
        }
        Ok(results)
    }

    pub fn search(&self, terms: &[String], full: bool) -> Result<SearchResults> {
        let regex = search_regex(terms)?;
        let mut lines = Vec::new();
        let mut files = BTreeSet::new();
        for path in self.files()? {
            let label = search_label(&path, full);
            let before = lines.len();
            lines.extend(
                self.read_lines(&path)?
                    .iter()
                    .filter(|line| regex.is_match(line))
                    .filter_map(|line| search_entry(line, full))
                    .map(|text| format!("{label}{text}")),
            );
            if lines.len() > before {
                files.insert(path);
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

fn section_prefix(section: &str) -> &str {
    if matches!(section, "0" | "00") {
        "0"
    } else {
        section
    }
}

fn list_includes(stem: &str, prefix: Option<&str>, sections_only: bool) -> bool {
    (!sections_only || stem.ends_with("00")) && prefix.is_none_or(|prefix| stem.starts_with(prefix))
}

fn list_entry(line: &str, sections_only: bool) -> Option<String> {
    if !line.contains("@topic") || (sections_only && !line.contains("~~~")) {
        return None;
    }
    non_blank(line.replacen("@topic", "", 1))
}

fn search_regex(terms: &[String]) -> Result<Regex> {
    let expression = if terms.is_empty() {
        ".*".into()
    } else {
        format!("({})", terms.join("|"))
    };
    RegexBuilder::new(&expression)
        .case_insensitive(true)
        .build()
        .context("invalid search expression")
}

/// Full searches label lines by file name; metadata searches by topic number.
fn search_label(path: &Path, full: bool) -> String {
    if full {
        format!(
            "{}: ",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    } else {
        format!("{}:", topic_stem(path).expect("validated topic"))
    }
}

fn search_entry(line: &str, full: bool) -> Option<String> {
    if full {
        non_blank(line.to_owned())
    } else if line.contains("@topic") || line.contains("@search") {
        non_blank(line.replacen("@topic", "", 1).replacen("@search", "", 1))
    } else {
        None
    }
}

fn non_blank(text: String) -> Option<String> {
    (!text.trim().is_empty()).then_some(text)
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
    fn lists_by_section() {
        let (_d, s) = fixture();
        assert_eq!(
            s.list(Some("01"), false).unwrap(),
            ["0100: ~~~ First section", "0101: Markdown topic"]
        );
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
        let all = s.search(&[], false).unwrap();
        assert_eq!(all.lines.len(), 5);
        assert_eq!(all.files.len(), 3);
        assert!(
            s.search(&["nomatch".into()], true)
                .unwrap()
                .files
                .is_empty()
        );
    }
    #[test]
    fn footer_requires_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0001.txt");
        fs::write(&path, "body\n").unwrap();
        assert!(update_footer(&path, "", "x@y").is_err());
    }

    fn footer_after(text: &str) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0001.md");
        fs::write(&path, text).unwrap();
        update_footer(&path, " Test User ", " test@example.com ").unwrap();
        let date = Local::now().format("%Y-%m-%d").to_string();
        fs::read_to_string(&path).unwrap().replace(&date, "DATE")
    }

    const FOOTER: &str = "\n---\nLast updated: DATE, by \"Test User <test@example.com>\"\n";

    #[test]
    fn footer_is_appended() {
        assert_eq!(footer_after("body\n\n\n"), format!("body\n{FOOTER}"));
        assert_eq!(footer_after(""), FOOTER);
    }

    #[test]
    fn footer_replaces_previous_footer() {
        assert_eq!(
            footer_after("body\n\n-----\n\nLast updated: 2020-01-01, by \"Old <o@x>\"\n\n"),
            format!("body\n{FOOTER}")
        );
        // Without a rule above it, only the old footer line is removed.
        assert_eq!(
            footer_after("body\nLast updated: long ago\ntail\n"),
            format!("body\ntail\n{FOOTER}")
        );
        // Only the last footer is replaced.
        assert_eq!(
            footer_after("Last updated: first\n---\nLast updated: second\n"),
            format!("Last updated: first\n{FOOTER}")
        );
    }

    #[test]
    fn footer_normalizes_line_endings() {
        assert_eq!(footer_after("a\r\nb\r\n"), format!("a\nb\n{FOOTER}"));
    }

    #[cfg(unix)]
    #[test]
    fn footer_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("0001.md");
        fs::write(&path, "body\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        update_footer(&path, "a", "b").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[test]
    fn validates_author() {
        assert!(validate_author("a", "b").is_ok());
        assert_eq!(
            validate_author(" ", "b").unwrap_err().to_string(),
            "author_name is required in the topic configuration"
        );
        assert_eq!(
            validate_author("a", "").unwrap_err().to_string(),
            "author_email is required in the topic configuration"
        );
    }

    #[test]
    fn horizontal_rules_need_three_dashes() {
        assert!(is_horizontal_rule("---"));
        assert!(is_horizontal_rule("  -----  "));
        assert!(!is_horizontal_rule("--"));
        assert!(!is_horizontal_rule("-- -"));
        assert!(!is_horizontal_rule(""));
    }

    #[test]
    fn topic_stems() {
        for (name, stem) in [
            ("0001.txt", Some("0001")),
            ("0001.TXT", Some("0001")),
            ("9999.md", Some("9999")),
            ("0002.MD.GPG", Some("0002")),
            ("dir/0003.md", Some("0003")),
            ("001.txt", None),
            ("abcd.txt", None),
            ("0001.rs", None),
            ("0001", None),
        ] {
            assert_eq!(topic_stem(Path::new(name)).as_deref(), stem, "{name}");
        }
    }

    #[test]
    fn find_accepts_numbers_and_file_names() {
        let (_d, s) = fixture();
        for value in [
            "1",
            "0001",
            "0001.txt",
            "some/dir/0001.txt",
            "0001.anything",
        ] {
            assert_eq!(
                s.find(value).unwrap().file_name().unwrap(),
                "0001.txt",
                "{value}"
            );
        }
        assert_eq!(s.find("101").unwrap().file_name().unwrap(), "0101.md");
    }

    #[test]
    fn find_rejects_missing_and_invalid_topics() {
        let (_d, s) = fixture();
        for value in ["2", "10000", "70000", "-1", "abc", ""] {
            assert_eq!(
                s.find(value).unwrap_err().to_string(),
                format!("{value}: no topic found")
            );
        }
    }

    #[test]
    fn find_rejects_ambiguous_topics() {
        let (d, s) = fixture();
        fs::write(d.path().join("0001.md"), "").unwrap();
        let error = s.find("1").unwrap_err().to_string();
        assert!(error.starts_with("1: multiple topic files found: "));
        assert!(error.contains("0001.txt") && error.contains("0001.md"));
    }

    #[test]
    fn files_skip_directories_and_other_names() {
        let (d, s) = fixture();
        fs::create_dir(d.path().join("0200.md")).unwrap();
        let names: Vec<_> = s
            .files()
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["0001.txt", "0100.md", "0101.md"]);
    }

    #[test]
    fn missing_directory_is_reported() {
        let s = Store {
            dir: "/nonexistent/topics".into(),
            gpg_binary: "gpg".into(),
        };
        assert!(
            s.files()
                .unwrap_err()
                .to_string()
                .starts_with("read topic directory")
        );
    }

    #[test]
    fn lists_all_and_section_zero() {
        let (d, s) = fixture();
        assert_eq!(
            s.list(None, false).unwrap(),
            [
                "0001: First topic",
                "0100: ~~~ First section",
                "0101: Markdown topic"
            ]
        );
        fs::write(d.path().join("0000.md"), "@topic ~~~ Index\n@topic\n").unwrap();
        fs::write(d.path().join("1000.md"), "@topic Thousands\n").unwrap();
        // Section "0" and "00" both cover every topic below 1000.
        for section in ["0", "00"] {
            assert_eq!(
                s.list(Some(section), false).unwrap(),
                [
                    "0000: ~~~ Index",
                    "0001: First topic",
                    "0100: ~~~ First section",
                    "0101: Markdown topic"
                ]
            );
        }
    }

    #[test]
    fn search_is_case_insensitive_and_ors_terms() {
        let (_d, s) = fixture();
        let results = s.search(&["ALPHA".into(), "gamma".into()], false).unwrap();
        assert_eq!(results.lines, ["0001: alpha beta", "0101: gamma"]);
        assert_eq!(
            results.files,
            [s.dir.join("0001.txt"), s.dir.join("0101.md")]
        );
        // Metadata searches ignore body text.
        assert!(
            s.search(&["needle".into()], false)
                .unwrap()
                .lines
                .is_empty()
        );
    }

    #[test]
    fn invalid_search_expression_is_reported() {
        let (_d, s) = fixture();
        assert_eq!(
            s.search(&["(".into()], false).err().unwrap().to_string(),
            "invalid search expression"
        );
    }

    #[test]
    fn read_topic_frames_content() {
        let (d, s) = fixture();
        fs::write(d.path().join("0002.md"), "no newline").unwrap();
        assert_eq!(
            s.read_topic(&d.path().join("0002.md")).unwrap(),
            format!(
                "\n-- 0002.md {}\nno newline\n{}\n\n",
                "-".repeat(72),
                "-".repeat(80)
            )
        );
        fs::write(d.path().join("0003.md"), [0xff, 0xfe]).unwrap();
        assert_eq!(
            s.read_topic(&d.path().join("0003.md"))
                .unwrap_err()
                .to_string(),
            "topic is not UTF-8"
        );
    }

    #[test]
    fn missing_gpg_is_reported() {
        let (d, mut s) = fixture();
        s.gpg_binary = "/nonexistent/gpg".into();
        let encrypted = d.path().join("0005.gpg");
        fs::write(&encrypted, "").unwrap();
        assert!(
            s.read_data(&encrypted)
                .unwrap_err()
                .to_string()
                .contains("was not found")
        );
        s.gpg_binary = "no-such-gpg-binary".into();
        assert!(
            s.encrypt(&encrypted, &encrypted, &["a".into()])
                .unwrap_err()
                .to_string()
                .contains("was not found")
        );
    }

    #[test]
    fn run_interactive_reports_failure() {
        let dir = tempfile::tempdir().unwrap();
        assert!(run_interactive(dir.path(), "true", &[]).is_ok());
        assert!(
            run_interactive(dir.path(), "false", &["x"])
                .unwrap_err()
                .to_string()
                .starts_with("false x: exited with")
        );
    }
}
