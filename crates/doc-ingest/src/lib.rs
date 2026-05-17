use anyhow::{Context, Result};
use common_utils::{
    list_files_with_exts, markdown_excerpt, markdown_title, parse_front_matter, read_toml_file,
    render_markdown_document, slugify, today_string, write_if_changed,
};
use core_types::{ContentSource, DocIngestReport, PostFrontMatter, SiteConfig};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialPostFrontMatter {
    title: Option<String>,
    slug: Option<String>,
    date: Option<String>,
    updated: Option<String>,
    summary: Option<String>,
    tags: Option<Vec<String>>,
    category: Option<String>,
    hero_image: Option<String>,
    hero_image_prompt: Option<String>,
    hero_alt: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DocxDocument {
    pub title: String,
    pub markdown: String,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct Paragraph {
    text: String,
    style: Option<String>,
    is_list: bool,
}

#[derive(Debug, Clone)]
enum Block {
    Paragraph(Paragraph),
    Table(Vec<Vec<String>>),
}

pub fn ingest_documents(
    config_path: impl AsRef<Path>,
    docs_dir: impl AsRef<Path>,
    docx_dir: impl AsRef<Path>,
) -> Result<DocIngestReport> {
    let config: SiteConfig = read_toml_file(config_path)?;
    let mut report = DocIngestReport {
        scanned: 0,
        written: 0,
        outputs: Vec::new(),
    };

    for path in list_files_with_exts(docs_dir, &["md"])? {
        report.scanned += 1;
        if let Some(output) = ingest_markdown_file(&config, &path)? {
            report.written += 1;
            report.outputs.push(output);
        }
    }

    for path in list_files_with_exts(docx_dir, &["docx"])? {
        report.scanned += 1;
        if let Some(output) = ingest_docx_file(&config, &path)? {
            report.written += 1;
            report.outputs.push(output);
        }
    }

    Ok(report)
}

fn ingest_markdown_file(config: &SiteConfig, path: &Path) -> Result<Option<String>> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let (partial, body): (Option<PartialPostFrontMatter>, String) = parse_front_matter(&raw)?;
    let partial = partial.unwrap_or_default();
    let title = partial
        .title
        .clone()
        .or_else(|| markdown_title(&body))
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "Imported document".to_string());
    let slug = partial.slug.clone().unwrap_or_else(|| slugify(&title));
    let status = normalize_status(
        partial
            .status
            .as_deref()
            .unwrap_or(&config.publish.default_status),
    );
    let date = partial.date.clone().unwrap_or_else(today_string);
    let front = PostFrontMatter {
        title,
        slug: slug.clone(),
        date: date.clone(),
        updated: partial.updated.clone().unwrap_or(date),
        summary: partial
            .summary
            .clone()
            .unwrap_or_else(|| markdown_excerpt(&body, 160)),
        tags: partial.tags.unwrap_or_default(),
        category: partial.category.unwrap_or_else(|| "Imported".to_string()),
        hero_image: partial.hero_image,
        hero_image_prompt: partial.hero_image_prompt,
        hero_alt: partial.hero_alt,
        status: status.clone(),
        source: Some(ContentSource {
            kind: "markdown-import".to_string(),
            path: path.to_string_lossy().to_string(),
        }),
    };

    let output = target_dir(config, &status).join(format!("{slug}.md"));
    let rendered = render_markdown_document(&front, &body)?;
    if write_if_changed(&output, rendered.as_bytes())? {
        Ok(Some(output.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

fn ingest_docx_file(config: &SiteConfig, path: &Path) -> Result<Option<String>> {
    let slug = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(slugify)
        .unwrap_or_else(|| "docx-import".to_string());
    let image_dir = PathBuf::from("static/images/imported").join(&slug);
    let document = parse_docx(path, &image_dir, config.docx.extract_images)?;
    let status = normalize_status(&config.docx.publish_mode);
    let date = today_string();
    let front = PostFrontMatter {
        title: if document.title.is_empty() {
            slug.clone()
        } else {
            document.title.clone()
        },
        slug: slug.clone(),
        date: date.clone(),
        updated: date,
        summary: markdown_excerpt(&document.markdown, 160),
        tags: Vec::new(),
        category: "Imported".to_string(),
        hero_image: None,
        hero_image_prompt: None,
        hero_alt: None,
        status: status.clone(),
        source: Some(ContentSource {
            kind: "docx-import".to_string(),
            path: path.to_string_lossy().to_string(),
        }),
    };
    let output = target_dir(config, &status).join(format!("{slug}.md"));
    let rendered = render_markdown_document(&front, &document.markdown)?;
    if write_if_changed(&output, rendered.as_bytes())? {
        Ok(Some(output.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

pub fn parse_docx(
    path: &Path,
    image_output_dir: &Path,
    extract_images: bool,
) -> Result<DocxDocument> {
    let file = File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut archive = ZipArchive::new(file).context("failed to read DOCX zip container")?;

    let mut xml = String::new();
    {
        let mut document = archive
            .by_name("word/document.xml")
            .context("DOCX is missing word/document.xml")?;
        document
            .read_to_string(&mut xml)
            .context("failed to read word/document.xml")?;
    }

    let images = if extract_images {
        extract_docx_images(&mut archive, image_output_dir)?
    } else {
        Vec::new()
    };
    let (title, mut markdown) = parse_docx_document_xml(&xml)?;
    if !images.is_empty() {
        markdown.push_str("\n\n## 첨부 이미지\n\n");
        for image in &images {
            markdown.push_str(&format!(
                "![Imported image](/images/imported/{})\n\n",
                image
            ));
        }
    }

    Ok(DocxDocument {
        title,
        markdown,
        images,
    })
}

pub fn parse_docx_document_xml(xml: &str) -> Result<(String, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut blocks = Vec::new();
    let mut current_para: Option<Paragraph> = None;
    let mut current_text = String::new();
    let mut in_text = false;
    let mut in_table = false;
    let mut in_cell = false;
    let mut cell_text = String::new();
    let mut row: Vec<String> = Vec::new();
    let mut table: Vec<Vec<String>> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(event) => match local_name(event.name().as_ref()) {
                b"p" => current_para = Some(Paragraph::default()),
                b"t" => {
                    in_text = true;
                    current_text.clear();
                }
                b"tbl" => {
                    in_table = true;
                    table.clear();
                }
                b"tr" => row.clear(),
                b"tc" => {
                    in_cell = true;
                    cell_text.clear();
                }
                b"pStyle" => set_paragraph_style(&reader, &event, current_para.as_mut())?,
                b"numPr" => {
                    if let Some(paragraph) = current_para.as_mut() {
                        paragraph.is_list = true;
                    }
                }
                _ => {}
            },
            Event::Empty(event) => match local_name(event.name().as_ref()) {
                b"pStyle" => set_paragraph_style(&reader, &event, current_para.as_mut())?,
                b"numPr" => {
                    if let Some(paragraph) = current_para.as_mut() {
                        paragraph.is_list = true;
                    }
                }
                _ => {}
            },
            Event::Text(event) => {
                if in_text {
                    current_text.push_str(&event.unescape()?.into_owned());
                }
            }
            Event::End(event) => match local_name(event.name().as_ref()) {
                b"t" => {
                    if let Some(paragraph) = current_para.as_mut() {
                        paragraph.text.push_str(&current_text);
                    }
                    in_text = false;
                    current_text.clear();
                }
                b"p" => {
                    if let Some(paragraph) = current_para.take() {
                        let text = paragraph.text.trim();
                        if !text.is_empty() {
                            if in_table && in_cell {
                                if !cell_text.is_empty() {
                                    cell_text.push(' ');
                                }
                                cell_text.push_str(text);
                            } else {
                                blocks.push(Block::Paragraph(paragraph));
                            }
                        }
                    }
                }
                b"tc" => {
                    in_cell = false;
                    row.push(cell_text.trim().to_string());
                    cell_text.clear();
                }
                b"tr" => {
                    if !row.is_empty() {
                        table.push(row.clone());
                    }
                }
                b"tbl" => {
                    in_table = false;
                    if !table.is_empty() {
                        blocks.push(Block::Table(table.clone()));
                    }
                    table.clear();
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let title = blocks
        .iter()
        .find_map(|block| match block {
            Block::Paragraph(paragraph) if is_title_style(paragraph.style.as_deref()) => {
                Some(paragraph.text.trim().to_string())
            }
            _ => None,
        })
        .or_else(|| {
            blocks.iter().find_map(|block| match block {
                Block::Paragraph(paragraph) => Some(paragraph.text.trim().to_string()),
                _ => None,
            })
        })
        .unwrap_or_else(|| "Imported DOCX".to_string());

    let mut markdown = String::new();
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => push_paragraph_markdown(&mut markdown, &paragraph),
            Block::Table(rows) => push_table_markdown(&mut markdown, &rows),
        }
    }

    Ok((title, markdown.trim().to_string()))
}

fn extract_docx_images(
    archive: &mut ZipArchive<File>,
    image_output_dir: &Path,
) -> Result<Vec<String>> {
    let mut images = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_string();
        if !name.starts_with("word/media/") {
            continue;
        }
        fs::create_dir_all(image_output_dir)
            .with_context(|| format!("failed to create {}", image_output_dir.display()))?;
        let filename = Path::new(&name)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("image.bin");
        let output = image_output_dir.join(filename);
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        let mut file = File::create(&output)
            .with_context(|| format!("failed to create {}", output.display()))?;
        file.write_all(&bytes)?;
        let relative = output
            .strip_prefix("static/images/imported")
            .unwrap_or(&output)
            .to_string_lossy()
            .trim_start_matches('/')
            .to_string();
        images.push(relative);
    }
    Ok(images)
}

fn set_paragraph_style(
    reader: &Reader<&[u8]>,
    event: &BytesStart<'_>,
    paragraph: Option<&mut Paragraph>,
) -> Result<()> {
    let Some(paragraph) = paragraph else {
        return Ok(());
    };
    for attribute in event.attributes().flatten() {
        if local_name(attribute.key.as_ref()) == b"val" {
            paragraph.style = Some(
                attribute
                    .decode_and_unescape_value(reader.decoder())?
                    .into_owned(),
            );
        }
    }
    Ok(())
}

fn push_paragraph_markdown(markdown: &mut String, paragraph: &Paragraph) {
    let text = paragraph.text.trim();
    if text.is_empty() {
        return;
    }
    if let Some(style) = paragraph.style.as_deref() {
        if is_title_style(Some(style)) {
            markdown.push_str(&format!("# {text}\n\n"));
            return;
        }
        if let Some(level) = heading_level(style) {
            markdown.push_str(&format!("{} {text}\n\n", "#".repeat(level)));
            return;
        }
    }
    if paragraph.is_list {
        markdown.push_str(&format!("- {text}\n"));
    } else {
        markdown.push_str(text);
        markdown.push_str("\n\n");
    }
}

fn push_table_markdown(markdown: &mut String, rows: &[Vec<String>]) {
    if rows.is_empty() {
        return;
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    if width == 0 {
        return;
    }
    let header = normalize_row(&rows[0], width);
    markdown.push('|');
    for cell in &header {
        markdown.push_str(&format!(" {} |", escape_table_cell(cell)));
    }
    markdown.push('\n');
    markdown.push('|');
    for _ in 0..width {
        markdown.push_str(" --- |");
    }
    markdown.push('\n');
    for row in rows.iter().skip(1) {
        markdown.push('|');
        for cell in normalize_row(row, width) {
            markdown.push_str(&format!(" {} |", escape_table_cell(&cell)));
        }
        markdown.push('\n');
    }
    markdown.push('\n');
}

fn normalize_row(row: &[String], width: usize) -> Vec<String> {
    let mut out = row.to_vec();
    out.resize_with(width, String::new);
    out
}

fn escape_table_cell(cell: &str) -> String {
    cell.replace('|', "\\|")
}

fn heading_level(style: &str) -> Option<usize> {
    let normalized = style.to_ascii_lowercase();
    normalized
        .strip_prefix("heading")
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .filter(|level| (1..=6).contains(level))
}

fn is_title_style(style: Option<&str>) -> bool {
    style
        .map(|value| {
            let value = value.to_ascii_lowercase();
            value == "title" || value == "heading1"
        })
        .unwrap_or(false)
}

fn local_name(name: &[u8]) -> &[u8] {
    name.iter()
        .position(|byte| *byte == b':')
        .map(|index| &name[index + 1..])
        .unwrap_or(name)
}

fn normalize_status(status: &str) -> String {
    match status {
        "publish" | "published" => "published".to_string(),
        _ => "draft".to_string(),
    }
}

fn target_dir(config: &SiteConfig, status: &str) -> PathBuf {
    if status == "published" {
        PathBuf::from(&config.build.posts_dir)
    } else {
        PathBuf::from("content/drafts")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn docx_xml_extracts_heading_list_and_table() {
        let xml = r#"
<w:document xmlns:w="w">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>문서 제목</w:t></w:r></w:p>
    <w:p><w:r><w:t>본문 단락</w:t></w:r></w:p>
    <w:p><w:pPr><w:numPr/></w:pPr><w:r><w:t>목록 항목</w:t></w:r></w:p>
    <w:tbl>
      <w:tr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr>
      <w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc></w:tr>
    </w:tbl>
  </w:body>
</w:document>"#;
        let (title, markdown) = parse_docx_document_xml(xml).unwrap();
        assert_eq!(title, "문서 제목");
        assert!(markdown.contains("# 문서 제목"));
        assert!(markdown.contains("- 목록 항목"));
        assert!(markdown.contains("| A | B |"));
    }

    #[test]
    fn markdown_import_writes_draft() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("imports/docs")).unwrap();
        fs::write(root.join("imports/docs/a.md"), "# Hello\n\nBody").unwrap();
        fs::write(
            root.join("site.toml"),
            format!(
                r#"title = "Test"
description = "Desc"
base_url = "/"
language = "ko-KR"
author = "Tester"

[build]
output_dir = "{}"
posts_dir = "{}"
assets_dir = "{}"

[publish]
default_status = "draft"
"#,
                root.join("public").display(),
                root.join("content/posts").display(),
                root.join("static").display()
            ),
        )
        .unwrap();
        let old = std::env::current_dir().unwrap();
        std::env::set_current_dir(root).unwrap();
        let report = ingest_documents(
            root.join("site.toml"),
            root.join("imports/docs"),
            root.join("imports/docx"),
        )
        .unwrap();
        std::env::set_current_dir(old).unwrap();
        assert_eq!(report.scanned, 1);
        assert!(root.join("content/drafts/hello.md").exists());
    }
}
