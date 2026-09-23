use crate::config::Config;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local};
use regex::Regex;
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub struct Engine {
    pub config: Config,
    pub now: DateTime<Local>,
}

#[derive(Default)]
pub struct CreateOptions {
    pub template: Option<String>,
    pub force: bool,
    pub backup: bool,
}

pub struct TemplateInfo {
    pub name: String,
    pub path: PathBuf,
}

impl Engine {
    pub fn templates(&self) -> Result<Vec<TemplateInfo>> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for dir in &self.config.template_dirs {
            let entries = match fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("read template directory {:?}", dir));
                }
            };
            for entry in entries {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.file_type()?.is_file() && seen.insert(name.clone()) {
                    result.push(TemplateInfo {
                        name,
                        path: entry.path(),
                    });
                }
            }
        }
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    pub fn find_template(&self, name: &str) -> Result<PathBuf> {
        if name.is_empty()
            || Path::new(name).file_name().and_then(|value| value.to_str()) != Some(name)
        {
            bail!("invalid template name {name:?}");
        }
        self.config
            .template_dirs
            .iter()
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
            .with_context(|| format!("template {name:?} was not found"))
    }

    pub fn create(
        &self,
        target: &Path,
        options: &CreateOptions,
        input: &mut dyn BufRead,
        output: &mut dyn Write,
    ) -> Result<()> {
        if options.template.is_none()
            && let Some(base) = target.file_name().and_then(|value| value.to_str())
            && let Ok(template) = self.find_template(base)
        {
            return self.generate(&template, target, options, input, output);
        }
        for (target, template_name) in target_plans(target, options.template.as_deref())? {
            let template = self
                .find_template(&template_name)
                .with_context(|| format!("create {:?}", target))?;
            self.generate(&template, &target, options, input, output)?;
        }
        Ok(())
    }

    fn generate(
        &self,
        template: &Path,
        target: &Path,
        options: &CreateOptions,
        input: &mut dyn BufRead,
        output: &mut dyn Write,
    ) -> Result<()> {
        let content = fs::read_to_string(template)?;
        let rendered = self.render(&content, template, target);
        if !prepare_target(target, options, input, output)? {
            return Ok(());
        }
        if let Some(parent) = target.parent().filter(|path| !path.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, rendered).with_context(|| format!("write {:?}", target))?;
        writeln!(
            output,
            "Created {} from {}",
            target.display(),
            template.display()
        )?;
        Ok(())
    }

    pub fn render(&self, content: &str, template: &Path, target: &Path) -> String {
        let (exact, insensitive) = self.substitutions(template, target);
        let pattern = Regex::new(r"%%([^%\r\n]+)%%").expect("valid tag pattern");
        pattern
            .replace_all(content, |captures: &regex::Captures<'_>| {
                let tag = &captures[0];
                let key = &captures[1];
                exact
                    .get(key)
                    .or_else(|| insensitive.get(&key.to_lowercase()))
                    .cloned()
                    .unwrap_or_else(|| tag.to_owned())
            })
            .into_owned()
    }

    pub fn show_substitutions(&self, template: &Path, target: &Path) -> Vec<String> {
        let (exact, insensitive) = self.substitutions(template, target);
        let mut lines: Vec<_> = exact
            .into_iter()
            .chain(insensitive)
            .map(|(key, value)| format!("{:<22} -> {value}", format!("%%{key}%%")))
            .collect();
        lines.sort();
        lines
    }

    fn substitutions(
        &self,
        template: &Path,
        target: &Path,
    ) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
        let filename = target
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let extension = target
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let basename = target
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let lower_filename = filename.to_lowercase();
        let lower_extension = extension.to_lowercase();
        let lower_basename = basename.to_lowercase();
        let exact = BTreeMap::from([
            ("fIlename".into(), filename.clone()),
            ("bAsename".into(), basename),
            ("filename".into(), lower_filename),
            ("extension".into(), lower_extension),
            ("basename".into(), lower_basename.clone()),
            ("Basename".into(), upper_first(&lower_basename)),
            ("FILENAME".into(), filename.to_uppercase()),
            ("EXTENSION".into(), extension.to_uppercase()),
            ("BASENAME".into(), lower_basename.to_uppercase()),
        ]);
        let file_header: String = format!("//~~ {filename} {}", "~".repeat(120))
            .chars()
            .take(80)
            .collect();
        let mut header_dashes = format!("/*== {filename} {}", "=".repeat(120));
        if header_dashes.chars().count() > 80 {
            header_dashes = header_dashes.chars().take(78).collect();
            header_dashes.push_str("*\\");
        }
        let utc = self.now.to_utc();
        let user = env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or_default();
        let author = &self.config.author_name;
        let email = &self.config.author_email;
        let mut insensitive = BTreeMap::from([
            ("fileheader".into(), file_header),
            ("header-dashes".into(), header_dashes),
            (
                "template".into(),
                template
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            ("templatepath".into(), template.display().to_string()),
            ("shortdate".into(), self.now.format("%m/%d/%y").to_string()),
            ("date".into(), self.now.format("%b %d, %Y").to_string()),
            (
                "longdate".into(),
                self.now.format("%A %B %d, %Y").to_string(),
            ),
            ("yymmdd".into(), self.now.format("%y%m%d").to_string()),
            ("ccyymmdd".into(), self.now.format("%Y%m%d").to_string()),
            ("yy-mm-dd".into(), self.now.format("%y-%m-%d").to_string()),
            ("ccyy-mm-dd".into(), self.now.format("%Y-%m-%d").to_string()),
            (
                "iso8601date".into(),
                self.now.format("%Y%m%dT%H%M%S").to_string(),
            ),
            (
                "utc8601date".into(),
                utc.format("%Y%m%dT%H%M%SZ").to_string(),
            ),
            ("time".into(), self.now.format("%H:%M:%S").to_string()),
            (
                "longtime".into(),
                self.now.format("%H:%M:%S %z").to_string(),
            ),
            (
                "datetime".into(),
                self.now.format("%Y-%m-%d %H:%M:%S %Z").to_string(),
            ),
            (
                "utcdatetime".into(),
                utc.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            ),
            ("author".into(), author.clone()),
            ("auther".into(), author.clone()),
            ("email".into(), email.clone()),
            ("fullname".into(), author.clone()),
            ("authorname".into(), author.clone()),
            ("authoremail".into(), format!("<{email}>")),
            ("authorfull".into(), format!("{author} <{email}>")),
            ("user".into(), user),
            ("year".into(), self.now.format("%Y").to_string()),
            ("identity".into(), format!("$Id: {filename} $")),
            ("revision".into(), "1.0".into()),
            ("copyright".into(), "Copyright (c)".into()),
        ]);
        for (key, value) in &self.config.substitutions {
            insensitive.insert(key.to_lowercase(), value.clone());
        }
        (exact, insensitive)
    }
}

pub fn target_plans(target: &Path, explicit: Option<&str>) -> Result<Vec<(PathBuf, String)>> {
    if let Some(template) = explicit {
        return Ok(vec![(target.to_owned(), template.to_owned())]);
    }
    let base = target
        .file_name()
        .and_then(|value| value.to_str())
        .with_context(|| format!("invalid target {:?}", target))?;
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if extension.is_empty() {
        return Ok(vec![(target.to_owned(), base.to_owned())]);
    }
    let stem = target
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(base);
    let parent = target.parent().unwrap_or_else(|| Path::new(""));
    match extension {
        "ch" => Ok(vec![
            (parent.join(format!("{stem}.c")), "c".into()),
            (parent.join(format!("{stem}.h")), "h".into()),
        ]),
        "cchh" => Ok(vec![
            (parent.join(format!("{stem}.cc")), "cc".into()),
            (parent.join(format!("{stem}.hh")), "hh".into()),
        ]),
        other => Ok(vec![(target.to_owned(), other.to_owned())]),
    }
}

fn prepare_target(
    path: &Path,
    options: &CreateOptions,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    if options.force {
        return Ok(true);
    }
    let backup = options.backup || {
        write!(
            output,
            "{} exists. Abort, overwrite, or back up [A/o/b]? ",
            path.display()
        )?;
        output.flush()?;
        let mut answer = String::new();
        input.read_line(&mut answer)?;
        match answer.trim().to_lowercase().as_str() {
            "o" | "overwrite" => return Ok(true),
            "b" | "backup" | "move" | "m" => true,
            _ => return Ok(false),
        }
    };
    if backup {
        let backup_path = PathBuf::from(format!("{}~", path.display()));
        if backup_path.exists() {
            fs::remove_file(&backup_path)?;
        }
        fs::rename(path, &backup_path).with_context(|| format!("back up {:?}", path))?;
    }
    Ok(true)
}

fn upper_first(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::io::Cursor;

    fn engine(config: Config) -> Engine {
        Engine {
            config,
            now: Local.with_ymd_and_hms(2026, 9, 8, 14, 3, 2).unwrap(),
        }
    }

    #[test]
    fn plans_compound_targets() {
        assert_eq!(
            target_plans(Path::new("pair.ch"), None).unwrap(),
            vec![
                (PathBuf::from("pair.c"), "c".into()),
                (PathBuf::from("pair.h"), "h".into())
            ]
        );
        assert_eq!(
            target_plans(Path::new("pair.cchh"), None).unwrap(),
            vec![
                (PathBuf::from("pair.cc"), "cc".into()),
                (PathBuf::from("pair.hh"), "hh".into())
            ]
        );
    }

    #[test]
    fn renders_legacy_and_current_tags() {
        let mut config = Config {
            author_name: "Andy Pippin".into(),
            author_email: "andy@example.com".into(),
            ..Config::default()
        };
        config.substitutions.insert("revision".into(), "2.0".into());
        let rendered = engine(config).render("%%fIlename%%|%%bAsename%%|%%filename%%|%%basename%%|%%Basename%%|%%BASENAME%%|%%LongDate%%|%%authorFull%%|%%revision%%|%%unknown%%", Path::new("/templates/go"), Path::new("MyFile.Go"));
        for expected in [
            "MyFile.Go",
            "MyFile",
            "myfile.go",
            "myfile",
            "Myfile",
            "MYFILE",
            "Tuesday September 08, 2026",
            "Andy Pippin <andy@example.com>",
            "2.0",
            "%%unknown%%",
        ] {
            assert!(
                rendered.contains(expected),
                "missing {expected:?} in {rendered:?}"
            );
        }
    }

    #[test]
    fn creates_backup() {
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("templates");
        fs::create_dir(&templates).unwrap();
        fs::write(
            templates.join("txt"),
            "Name=%%fIlename%% Author=%%author%%\n",
        )
        .unwrap();
        let target = dir.path().join("Note.txt");
        fs::write(&target, "old").unwrap();
        let config = Config {
            template_dirs: vec![templates],
            author_name: "Andy".into(),
            ..Config::default()
        };
        engine(config)
            .create(
                &target,
                &CreateOptions {
                    backup: true,
                    ..CreateOptions::default()
                },
                &mut Cursor::new(Vec::new()),
                &mut Vec::new(),
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(format!("{}~", target.display())).unwrap(),
            "old"
        );
        assert!(
            fs::read_to_string(target)
                .unwrap()
                .contains("Name=Note.txt Author=Andy")
        );
    }

    #[test]
    fn template_directories_use_precedence_and_deduplicate() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        fs::write(first.join("go"), "first").unwrap();
        fs::write(second.join("go"), "second").unwrap();
        fs::write(second.join("sh"), "shell").unwrap();
        let engine = engine(Config {
            template_dirs: vec![first.clone(), second],
            ..Config::default()
        });
        assert_eq!(engine.find_template("go").unwrap(), first.join("go"));
        let templates = engine.templates().unwrap();
        assert_eq!(
            templates
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            ["go", "sh"]
        );
    }

    #[test]
    fn exact_target_template_precedes_extension() {
        let dir = tempfile::tempdir().unwrap();
        let templates = dir.path().join("templates");
        fs::create_dir(&templates).unwrap();
        fs::write(templates.join("html"), "extension").unwrap();
        fs::write(templates.join("Makefile.html"), "exact").unwrap();
        let target = dir.path().join("Makefile.html");
        let engine = engine(Config {
            template_dirs: vec![templates],
            ..Config::default()
        });
        engine
            .create(
                &target,
                &CreateOptions::default(),
                &mut Cursor::new(Vec::new()),
                &mut Vec::new(),
            )
            .unwrap();
        assert_eq!(fs::read_to_string(target).unwrap(), "exact");
    }
}
