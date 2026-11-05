use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

const WIDTH: usize = 80;
const MARKUP: [(&str, &str); 11] = [
    ("<i>", ""),
    ("</i>", ""),
    ("<b>", ""),
    ("</b>", ""),
    ("<u>", ""),
    ("</u>", ""),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&#39;", "'"),
    ("&amp;", "&"),
];

/// Converts each SRT file in `source` to plain text in `dir`, keeping the
/// language in the name so that several languages don't collide.
pub fn save(source: &Path, dir: &Path) -> Result<Vec<PathBuf>> {
    let mut saved = Vec::new();
    for entry in fs::read_dir(source).with_context(|| format!("read {:?}", source))? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "srt") {
            continue;
        }
        let text = fs::read_to_string(&path).with_context(|| format!("read {:?}", path))?;
        let name = path.with_extension("txt");
        let target = dir.join(name.file_name().context("subtitle file has no name")?);
        fs::create_dir_all(dir).with_context(|| format!("create {:?}", dir))?;
        fs::write(&target, clean(&text)).with_context(|| format!("write {:?}", target))?;
        saved.push(target);
    }
    if saved.is_empty() {
        bail!("no transcript available");
    }
    saved.sort();
    Ok(saved)
}

// Auto-generated captions roll, so each line appears in two blocks in a row.
fn clean(srt: &str) -> String {
    let lines: Vec<&str> = srt.lines().map(str::trim).collect();
    let mut captions: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let is_index = lines
            .get(index + 1)
            .is_some_and(|next| next.contains("-->"));
        if line.is_empty() || is_index || line.contains("-->") {
            continue;
        }
        let caption = strip_markup(line);
        if !caption.is_empty() && captions.last() != Some(&caption) {
            captions.push(caption);
        }
    }
    wrap(&captions.join(" "))
}

fn strip_markup(line: &str) -> String {
    let text = MARKUP
        .iter()
        .fold(line.to_owned(), |text, (from, to)| text.replace(from, to));
    text.trim().to_owned()
}

fn wrap(text: &str) -> String {
    let mut wrapped = String::new();
    let mut width = 0;
    for word in text.split_whitespace() {
        let length = word.chars().count();
        if width > 0 && width + 1 + length > WIDTH {
            wrapped.push('\n');
            width = 0;
        } else if width > 0 {
            wrapped.push(' ');
            width += 1;
        }
        wrapped.push_str(word);
        width += length;
    }
    wrapped.push('\n');
    wrapped
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROLLING: &str = "1\n00:00:01,200 --> 00:00:04,350\n \nRevenge with Karen.\n\n2\n00:00:04,350 --> 00:00:04,360\nRevenge with Karen.\n\n\n3\n00:00:04,360 --> 00:00:06,000\nRevenge with Karen.\nThe moment the new COO\n\n4\n00:00:06,000 --> 00:00:06,010\nThe moment the new COO\n";

    #[test]
    fn removes_timings_and_rolling_repeats() {
        assert_eq!(
            clean(ROLLING),
            "Revenge with Karen. The moment the new COO\n"
        );
    }

    #[test]
    fn keeps_captions_that_are_numbers() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\n1984\n\n2\n00:00:02,000 --> 00:00:03,000\nwas a book\n";
        assert_eq!(clean(srt), "1984 was a book\n");
    }

    #[test]
    fn handles_windows_line_endings() {
        let srt = "1\r\n00:00:01,000 --> 00:00:02,000\r\nHello\r\n\r\n";
        assert_eq!(clean(srt), "Hello\n");
    }

    #[test]
    fn repeats_apart_from_each_other_are_kept() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nYes\nNo\nYes\n";
        assert_eq!(clean(srt), "Yes No Yes\n");
    }

    #[test]
    fn removes_markup() {
        let srt =
            "1\n00:00:01,000 --> 00:00:02,000\n<i>Tom &amp; Jerry&#39;s</i> &lt;tag&gt; &amp;lt;\n";
        assert_eq!(clean(srt), "Tom & Jerry's <tag> &lt;\n");
    }

    #[test]
    fn wraps_long_text() {
        let wrapped = wrap(&"word ".repeat(40));
        assert!(wrapped.lines().count() > 1);
        assert!(wrapped.lines().all(|line| line.len() <= WIDTH));
        assert_eq!(wrap("short text"), "short text\n");
        assert_eq!(wrap(&"é".repeat(WIDTH)).lines().count(), 1);
    }

    #[test]
    fn saves_each_subtitle_file_as_text() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let dir = output.path().join("new/folder");
        fs::write(source.path().join("A 1.5 [x].en.srt"), ROLLING).unwrap();
        fs::write(source.path().join("A 1.5 [x].fr.srt"), ROLLING).unwrap();
        fs::write(source.path().join("ignored.json"), "{}").unwrap();
        assert_eq!(
            save(source.path(), &dir).unwrap(),
            [dir.join("A 1.5 [x].en.txt"), dir.join("A 1.5 [x].fr.txt")]
        );
        assert!(
            fs::read_to_string(dir.join("A 1.5 [x].en.txt"))
                .unwrap()
                .starts_with("Revenge with Karen.")
        );
        assert!(!dir.join("ignored.txt").exists());
    }

    #[test]
    fn reports_missing_transcript() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let dir = output.path().join("unused");
        let error = save(source.path(), &dir).unwrap_err();
        assert_eq!(error.to_string(), "no transcript available");
        assert!(!dir.exists());
    }
}
