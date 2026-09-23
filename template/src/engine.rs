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

    /// A temp directory with a `templates` subdirectory holding the given
    /// files, and an engine configured to use it.
    fn fixture(templates: &[(&str, &str)]) -> (tempfile::TempDir, Engine) {
        let dir = tempfile::tempdir().unwrap();
        let template_dir = dir.path().join("templates");
        fs::create_dir(&template_dir).unwrap();
        for (name, content) in templates {
            fs::write(template_dir.join(name), content).unwrap();
        }
        let engine = engine(Config {
            template_dirs: vec![template_dir],
            ..Config::default()
        });
        (dir, engine)
    }

    fn create_with(
        engine: &Engine,
        target: &Path,
        options: &CreateOptions,
        answer: &str,
    ) -> (Result<()>, String) {
        let mut output = Vec::new();
        let result = engine.create(
            target,
            options,
            &mut Cursor::new(answer.as_bytes().to_vec()),
            &mut output,
        );
        (result, String::from_utf8(output).unwrap())
    }

    #[test]
    fn plans_simple_targets() {
        let plan = |target: &str, explicit| target_plans(Path::new(target), explicit).unwrap();
        assert_eq!(
            plan("Makefile", None),
            [("Makefile".into(), "Makefile".into())]
        );
        assert_eq!(
            plan("src/main.rs", None),
            [("src/main.rs".into(), "rs".into())]
        );
        assert_eq!(
            plan("notes.txt", Some("md")),
            [("notes.txt".into(), "md".into())]
        );
        assert_eq!(
            plan("src/pair.ch", None),
            [
                (PathBuf::from("src/pair.c"), "c".into()),
                (PathBuf::from("src/pair.h"), "h".into())
            ]
        );
        assert!(target_plans(Path::new(".."), None).is_err());
    }

    #[test]
    fn rejects_invalid_template_names() {
        let (_dir, engine) = fixture(&[("go", "")]);
        for name in ["", "..", "../go", "sub/go", "/go"] {
            assert_eq!(
                engine.find_template(name).unwrap_err().to_string(),
                format!("invalid template name {name:?}")
            );
        }
        assert_eq!(
            engine.find_template("rs").unwrap_err().to_string(),
            "template \"rs\" was not found"
        );
    }

    #[test]
    fn listing_skips_directories_and_missing_paths() {
        let (dir, mut engine) = fixture(&[("sh", "")]);
        fs::create_dir(dir.path().join("templates/nested")).unwrap();
        engine
            .config
            .template_dirs
            .insert(0, dir.path().join("missing"));
        let names: Vec<_> = engine
            .templates()
            .unwrap()
            .into_iter()
            .map(|item| item.name)
            .collect();
        assert_eq!(names, ["sh"]);
    }

    #[test]
    fn creates_compound_targets_and_parent_directories() {
        let (dir, engine) = fixture(&[("c", "source %%filename%%"), ("h", "header %%BASENAME%%")]);
        let target = dir.path().join("out/deep/pair.ch");
        let (result, output) = create_with(&engine, &target, &CreateOptions::default(), "");
        result.unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("out/deep/pair.c")).unwrap(),
            "source pair.c"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("out/deep/pair.h")).unwrap(),
            "header PAIR"
        );
        assert_eq!(output.matches("Created ").count(), 2);
        assert!(!target.exists());
    }

    #[test]
    fn explicit_template_overrides_extension() {
        let (dir, engine) = fixture(&[("txt", "text"), ("md", "markdown")]);
        let target = dir.path().join("notes.txt");
        let options = CreateOptions {
            template: Some("md".into()),
            ..CreateOptions::default()
        };
        create_with(&engine, &target, &options, "").0.unwrap();
        assert_eq!(fs::read_to_string(target).unwrap(), "markdown");
    }

    #[test]
    fn missing_template_names_the_target() {
        let (dir, engine) = fixture(&[]);
        let target = dir.path().join("x.zzz");
        let error = create_with(&engine, &target, &CreateOptions::default(), "")
            .0
            .unwrap_err();
        assert_eq!(error.to_string(), format!("create {target:?}"));
        assert_eq!(
            error.root_cause().to_string(),
            "template \"zzz\" was not found"
        );
    }

    #[test]
    fn existing_target_prompt_answers() {
        let (dir, engine) = fixture(&[("txt", "new")]);
        let target = dir.path().join("a.txt");
        let backup = dir.path().join("a.txt~");
        let options = CreateOptions::default();

        for abort in ["\n", "a\n", "nonsense\n", ""] {
            fs::write(&target, "old").unwrap();
            let (result, output) = create_with(&engine, &target, &options, abort);
            result.unwrap();
            assert!(output.ends_with("exists. Abort, overwrite, or back up [A/o/b]? "));
            assert_eq!(fs::read_to_string(&target).unwrap(), "old");
        }
        assert!(!backup.exists());

        for overwrite in ["o\n", "Overwrite\n"] {
            fs::write(&target, "old").unwrap();
            create_with(&engine, &target, &options, overwrite)
                .0
                .unwrap();
            assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        }
        assert!(!backup.exists());

        for back_up in ["b\n", "backup\n", "m\n", "MOVE\n"] {
            fs::write(&target, back_up).unwrap();
            create_with(&engine, &target, &options, back_up).0.unwrap();
            assert_eq!(fs::read_to_string(&target).unwrap(), "new");
            // Each backup replaces the previous one.
            assert_eq!(fs::read_to_string(&backup).unwrap(), back_up);
        }
    }

    #[test]
    fn force_overwrites_without_prompting() {
        let (dir, engine) = fixture(&[("txt", "new")]);
        let target = dir.path().join("a.txt");
        fs::write(&target, "old").unwrap();
        let options = CreateOptions {
            force: true,
            ..CreateOptions::default()
        };
        let (result, output) = create_with(&engine, &target, &options, "");
        result.unwrap();
        assert!(!output.contains("exists"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert!(!dir.path().join("a.txt~").exists());
    }

    #[test]
    fn renders_dates_and_identity() {
        let engine = engine(Config::default());
        let rendered = engine.render(
            "%%shortdate%% %%date%% %%yymmdd%% %%CCYYMMDD%% %%yy-mm-dd%% %%ccyy-mm-dd%% \
             %%iso8601date%% %%time%% %%year%% %%identity%% %%template%% %%templatepath%% \
             %%extension%% %%EXTENSION%% %%FILENAME%%",
            Path::new("/templates/go"),
            Path::new("dir/Main.Go"),
        );
        assert_eq!(
            rendered,
            "09/08/26 Sep 08, 2026 260908 20260908 26-09-08 2026-09-08 \
             20260908T140302 14:03:02 2026 $Id: Main.Go $ go /templates/go \
             go GO MAIN.GO"
        );
    }

    #[test]
    fn renders_builtin_defaults_and_overrides_case_insensitively() {
        let mut config = Config {
            author_email: "a@example.com".into(),
            ..Config::default()
        };
        let engine_defaults = engine(config.clone());
        assert_eq!(
            engine_defaults.render(
                "%%revision%%|%%COPYRIGHT%%|%%authorEmail%%",
                Path::new("t"),
                Path::new("x")
            ),
            "1.0|Copyright (c)|<a@example.com>"
        );
        config
            .substitutions
            .insert("Project".into(), "Shelf".into());
        config
            .substitutions
            .insert("copyright".into(), "(C) Me".into());
        assert_eq!(
            engine(config).render(
                "%%project%%|%%PROJECT%%|%%Copyright%%",
                Path::new("t"),
                Path::new("x")
            ),
            "Shelf|Shelf|(C) Me"
        );
    }

    #[test]
    fn leaves_malformed_tags_alone() {
        let rendered = engine(Config::default()).render(
            "%%%% %%file\nname%% %%filename %%filename%%",
            Path::new("t"),
            Path::new("a.txt"),
        );
        assert_eq!(rendered, "%%%% %%file\nname%% %%filename a.txt");
    }

    #[test]
    fn headers_are_eighty_columns() {
        let engine = engine(Config::default());
        // The non-ASCII names put a multi-byte character across the cut point.
        let names = [
            "a.c".to_owned(),
            "n".repeat(100),
            format!("{}é.txt", "a".repeat(72)),
            format!("{}é.txt", "a".repeat(74)),
        ];
        for name in &names {
            let prefix: String = name.chars().take(3).collect();
            let header = engine.render("%%fileheader%%", Path::new("t"), Path::new(name));
            assert_eq!(header.chars().count(), 80);
            assert!(header.starts_with(&format!("//~~ {prefix}")));
            let dashes = engine.render("%%header-dashes%%", Path::new("t"), Path::new(name));
            assert_eq!(dashes.chars().count(), 80);
            assert!(dashes.starts_with("/*== "));
            assert!(dashes.ends_with("*\\"));
        }
    }

    #[test]
    fn show_substitutions_is_sorted_and_includes_custom_values() {
        let mut config = Config::default();
        config.substitutions.insert("Zeta".into(), "last".into());
        let lines = engine(config).show_substitutions(Path::new("/t/go"), Path::new("x.go"));
        let mut sorted = lines.clone();
        sorted.sort();
        assert_eq!(lines, sorted);
        assert!(lines.contains(&format!("{:<22} -> last", "%%zeta%%")));
        assert!(lines.contains(&format!("{:<22} -> go", "%%template%%")));
    }

    #[test]
    fn upper_first_handles_edge_cases() {
        assert_eq!(upper_first(""), "");
        assert_eq!(upper_first("a"), "A");
        assert_eq!(upper_first("éclair"), "Éclair");
        assert_eq!(upper_first("ßx"), "SSx");
    }
}
