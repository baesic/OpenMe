use anyhow::{Context, Result};
use common_utils::{
    ensure_parent, list_files_with_exts, parse_front_matter, render_markdown_document, slugify,
    write_if_changed,
};
use core_types::{ImageBrief, PostFrontMatter};
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageBuffer, Rgba};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ImageBriefReport {
    pub scanned: usize,
    pub updated: usize,
    pub outputs: Vec<String>,
}

pub fn process_posts(
    posts_dir: impl AsRef<Path>,
    images_dir: impl AsRef<Path>,
) -> Result<ImageBriefReport> {
    let posts_dir = posts_dir.as_ref();
    let images_dir = images_dir.as_ref();
    fs::create_dir_all(images_dir)
        .with_context(|| format!("failed to create {}", images_dir.display()))?;

    let mut report = ImageBriefReport::default();
    for path in list_files_with_exts(posts_dir, &["md"])? {
        report.scanned += 1;
        if process_post(&path, images_dir)? {
            report.updated += 1;
            report.outputs.push(path.to_string_lossy().to_string());
        }
    }
    Ok(report)
}

fn process_post(path: &Path, images_dir: &Path) -> Result<bool> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let (front, body): (Option<PostFrontMatter>, String) = parse_front_matter(&raw)?;
    let Some(mut front) = front else {
        return Ok(false);
    };
    if front.status != "published" {
        return Ok(false);
    }

    let brief = build_image_brief(&front);
    let hero_path = images_dir.join(format!("{}.webp", front.slug));
    let social_path = images_dir.join(format!("{}-social.webp", front.slug));
    let mut changed = false;

    if !hero_path.exists() {
        create_or_copy_image(&front.slug, &hero_path, 1200, 675, false)?;
        changed = true;
    }
    if !social_path.exists() {
        create_or_copy_image(&front.slug, &social_path, 1200, 630, true)?;
        changed = true;
    }

    let metadata_path = images_dir.join(format!("{}.image.json", front.slug));
    let metadata = serde_json::to_string_pretty(&brief)?;
    if write_if_changed(&metadata_path, metadata.as_bytes())? {
        changed = true;
    }

    if front.hero_image.as_deref() != Some(brief.hero_image.as_str()) {
        front.hero_image = Some(brief.hero_image.clone());
        changed = true;
    }
    if front.hero_image_prompt.as_deref() != Some(brief.hero_image_prompt.as_str()) {
        front.hero_image_prompt = Some(brief.hero_image_prompt.clone());
        changed = true;
    }
    if front.hero_alt.as_deref() != Some(brief.hero_alt.as_str()) {
        front.hero_alt = Some(brief.hero_alt.clone());
        changed = true;
    }

    if changed {
        let rendered = render_markdown_document(&front, &body)?;
        write_if_changed(path, rendered.as_bytes())?;
    }
    Ok(changed)
}

pub fn build_image_brief(front: &PostFrontMatter) -> ImageBrief {
    let slug = if front.slug.trim().is_empty() {
        slugify(&front.title)
    } else {
        front.slug.clone()
    };
    let hero_prompt = front.hero_image_prompt.clone().unwrap_or_else(|| {
        format!(
            "16:9 editorial illustration for a Korean technical blog post titled '{}', category {}, clean workflow diagram atmosphere, Rust and WebAssembly inspired shapes, no readable text",
            front.title, front.category
        )
    });
    let hero_alt = front
        .hero_alt
        .clone()
        .unwrap_or_else(|| format!("{} 주제를 표현한 기술 블로그 대표 이미지", front.title));
    ImageBrief {
        slug: slug.clone(),
        hero_image_prompt: hero_prompt.clone(),
        hero_alt,
        social_card_prompt: format!("{hero_prompt}, social card crop, high contrast focal center"),
        hero_image: format!("/images/posts/{slug}.webp"),
        social_image: format!("/images/posts/{slug}-social.webp"),
    }
}

fn create_or_copy_image(
    slug: &str,
    output: &Path,
    width: u32,
    height: u32,
    social: bool,
) -> Result<()> {
    if let Ok(dir) = std::env::var("OPENME_IMAGE_RESPONSE_DIR") {
        let source = PathBuf::from(dir).join(output.file_name().unwrap_or_default());
        if source.exists() {
            ensure_parent(output)?;
            fs::copy(&source, output).with_context(|| {
                format!(
                    "failed to copy generated image {} to {}",
                    source.display(),
                    output.display()
                )
            })?;
            return Ok(());
        }
    }
    generate_placeholder_webp(slug, output, width, height, social)
}

pub fn generate_placeholder_webp(
    slug: &str,
    output: &Path,
    width: u32,
    height: u32,
    social: bool,
) -> Result<()> {
    ensure_parent(output)?;
    let seed = hash_slug(slug);
    let a = [
        24 + (seed & 0x3f) as u8,
        86 + ((seed >> 8) & 0x3f) as u8,
        76 + ((seed >> 16) & 0x3f) as u8,
    ];
    let b = [
        190 - ((seed >> 24) & 0x4f) as u8,
        170 - ((seed >> 32) & 0x4f) as u8,
        92 + ((seed >> 40) & 0x3f) as u8,
    ];
    let c = [
        34 + ((seed >> 12) & 0x4f) as u8,
        42 + ((seed >> 20) & 0x4f) as u8,
        50 + ((seed >> 28) & 0x4f) as u8,
    ];
    let mut buffer: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;
            let mut pixel = [
                lerp(a[0], b[0], fx),
                lerp(a[1], b[1], fy),
                lerp(a[2], b[2], (fx + fy) / 2.0),
                255,
            ];
            let band = ((x as i64 - y as i64 * 2).abs() % 360) < if social { 60 } else { 42 };
            if band {
                pixel[0] = blend(pixel[0], c[0], 0.45);
                pixel[1] = blend(pixel[1], c[1], 0.45);
                pixel[2] = blend(pixel[2], c[2], 0.45);
            }
            let dx = x as f32 - width as f32 * 0.72;
            let dy = y as f32 - height as f32 * 0.42;
            let radius = (width.min(height) as f32) * if social { 0.30 } else { 0.26 };
            if dx * dx + dy * dy < radius * radius {
                pixel[0] = blend(pixel[0], 245, 0.18);
                pixel[1] = blend(pixel[1], 248, 0.18);
                pixel[2] = blend(pixel[2], 238, 0.18);
            }
            buffer.put_pixel(x, y, Rgba(pixel));
        }
    }
    let file = fs::File::create(output)
        .with_context(|| format!("failed to create {}", output.display()))?;
    let encoder = WebPEncoder::new_lossless(file);
    encoder
        .encode(buffer.as_raw(), width, height, ExtendedColorType::Rgba8)
        .with_context(|| format!("failed to encode {}", output.display()))?;
    Ok(())
}

fn hash_slug(slug: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in slug.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)) as u8
}

fn blend(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 * (1.0 - t) + b as f32 * t) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_brief_has_expected_paths() {
        let front = PostFrontMatter {
            title: "Hello".to_string(),
            slug: "hello".to_string(),
            date: "2026-05-17".to_string(),
            updated: "2026-05-17".to_string(),
            summary: "Summary".to_string(),
            tags: vec![],
            category: "Test".to_string(),
            hero_image: None,
            hero_image_prompt: None,
            hero_alt: None,
            status: "published".to_string(),
            source: None,
        };
        let brief = build_image_brief(&front);
        assert_eq!(brief.hero_image, "/images/posts/hello.webp");
        assert!(brief.hero_alt.contains("Hello"));
    }

    #[test]
    fn placeholder_webp_is_written() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("hero.webp");
        generate_placeholder_webp("hello", &output, 320, 180, false).unwrap();
        assert!(output.metadata().unwrap().len() > 100);
    }
}
