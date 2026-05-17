use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn read_toml_file<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse TOML {}", path.display()))
}

pub fn read_yaml_file<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("failed to parse YAML {}", path.display()))
}

pub fn parse_front_matter<T: DeserializeOwned>(input: &str) -> Result<(Option<T>, String)> {
    if let Some((yaml, body)) = split_front_matter(input) {
        let value = serde_yaml::from_str(yaml).context("failed to parse Markdown front matter")?;
        Ok((Some(value), body.to_string()))
    } else {
        Ok((None, input.to_string()))
    }
}

pub fn split_front_matter(input: &str) -> Option<(&str, &str)> {
    let rest = input
        .strip_prefix("---\n")
        .or_else(|| input.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim() == "---" {
            let yaml = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return Some((yaml, body));
        }
        offset += line.len();
    }
    None
}

pub fn render_markdown_document<T: Serialize>(front_matter: &T, body: &str) -> Result<String> {
    let yaml = serde_yaml::to_string(front_matter).context("failed to serialize front matter")?;
    let yaml = yaml.trim_start_matches("---\n").trim();
    Ok(format!("---\n{}\n---\n\n{}", yaml, body.trim_start()))
}

pub fn write_if_changed(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> Result<bool> {
    let path = path.as_ref();
    ensure_parent(path)?;
    let content = content.as_ref();
    if fs::read(path)
        .map(|existing| existing == content)
        .unwrap_or(false)
    {
        return Ok(false);
    }
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
}

pub fn ensure_parent(path: impl AsRef<Path>) -> Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

pub fn clean_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    Ok(())
}

pub fn copy_dir_recursive(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();
    if !from.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(from).with_context(|| format!("failed to list {}", from.display()))? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir_recursive(&source, &target)?;
        } else {
            ensure_parent(&target)?;
            fs::copy(&source, &target).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source.display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

pub fn list_files_with_exts(dir: impl AsRef<Path>, exts: &[&str]) -> Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    collect_files(dir, exts, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(dir: &Path, exts: &[&str], files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to list {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, exts, files)?;
        } else if path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| {
                exts.iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(ext))
            })
            .unwrap_or(false)
        {
            files.push(path);
        }
    }
    Ok(())
}

pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if matches!(ch, ' ' | '-' | '_' | '/' | ':' | '.' | '|')
            && !last_dash
            && !out.is_empty()
        {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "post".to_string()
    } else {
        out
    }
}

pub fn markdown_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("# ")
            .or_else(|| line.strip_prefix("## "))
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty())
    })
}

pub fn markdown_excerpt(markdown: &str, max_chars: usize) -> String {
    let plain = markdown
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .map(|line| {
            line.replace(['`', '*', '_', '[', ']', '(', ')', '>'], "")
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if plain.chars().count() <= max_chars {
        plain
    } else {
        let mut clipped = plain.chars().take(max_chars).collect::<String>();
        clipped.push_str("...");
        clipped
    }
}

pub fn today_string() -> String {
    if let Ok(date) = std::env::var("OPENME_DATE") {
        if !date.trim().is_empty() {
            return date;
        }
    }

    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64 / 86_400)
        .unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

pub fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn normalize_web_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    if !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }
    normalized
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::PostFrontMatter;

    #[test]
    fn slug_generation_keeps_meaningful_text() {
        assert_eq!(
            slugify("Rust WASM: GitHub Pages!"),
            "rust-wasm-github-pages"
        );
        assert_eq!(slugify("자동 블로그"), "자동-블로그");
    }

    #[test]
    fn front_matter_round_trip() {
        let markdown = "---\ntitle: Test\nslug: test\ndate: 2026-05-17\nupdated: 2026-05-17\nsummary: Summary\ntags: []\ncategory: Test\nstatus: published\n---\n\n# Body";
        let (front, body): (Option<PostFrontMatter>, String) =
            parse_front_matter(markdown).unwrap();
        assert_eq!(front.unwrap().slug, "test");
        assert_eq!(body.trim(), "# Body");
    }

    #[test]
    fn unix_date_conversion_is_stable() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_590), (2026, 5, 17));
    }
}
