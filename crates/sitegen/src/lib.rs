use anyhow::{Context, Result};
use common_utils::{
    clean_dir, copy_dir_recursive, escape_html, list_files_with_exts, normalize_web_path,
    parse_front_matter, read_toml_file, slugify, write_if_changed,
};
use core_types::{PostFrontMatter, SearchItem, SiteConfig};
use pulldown_cmark::{html, Options, Parser};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SiteBuildReport {
    pub posts_rendered: usize,
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct RenderPost {
    fm: PostFrontMatter,
    markdown: String,
    html: String,
    toc: Vec<Heading>,
    url: String,
    reading_minutes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    pub level: usize,
    pub title: String,
    pub id: String,
}

pub fn build_site(config_path: impl AsRef<Path>) -> Result<SiteBuildReport> {
    let config_path = config_path.as_ref();
    let config: SiteConfig = read_toml_file(config_path)?;
    let output_dir = PathBuf::from(&config.build.output_dir);
    clean_dir(&output_dir)?;
    copy_dir_recursive(&config.build.assets_dir, &output_dir)?;

    let mut posts = load_posts(&config.build.posts_dir)?;
    posts.sort_by(|a, b| {
        b.fm.date
            .cmp(&a.fm.date)
            .then_with(|| a.fm.title.cmp(&b.fm.title))
    });

    render_home(&config, &posts, &output_dir)?;
    render_post_index(&config, &posts, &output_dir)?;
    render_posts(&config, &posts, &output_dir)?;
    render_taxonomies(&config, &posts, &output_dir)?;
    render_search(&config, &posts, &output_dir)?;
    render_about(&config, &output_dir)?;
    render_not_found(&config, &output_dir)?;
    render_rss(&config, &posts, &output_dir)?;
    render_sitemap(&config, &posts, &output_dir)?;
    render_search_index(&posts, &output_dir)?;

    Ok(SiteBuildReport {
        posts_rendered: posts.len(),
        output_dir,
    })
}

pub fn markdown_to_html_with_toc(markdown: &str) -> (String, Vec<Heading>) {
    let (prepared, headings) = prepare_headings(markdown);
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(&prepared, options);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    (out, headings)
}

fn load_posts(posts_dir: impl AsRef<Path>) -> Result<Vec<RenderPost>> {
    let mut posts = Vec::new();
    for path in list_files_with_exts(posts_dir, &["md"])? {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let (front, body): (Option<PostFrontMatter>, String) = parse_front_matter(&raw)?;
        let Some(front) = front else {
            continue;
        };
        if front.status != "published" {
            continue;
        }
        let (html, toc) = markdown_to_html_with_toc(&body);
        let words = body.split_whitespace().count();
        let reading_minutes = (words / 280).max(1);
        let url = format!("/posts/{}/", front.slug);
        posts.push(RenderPost {
            fm: front,
            markdown: body,
            html,
            toc,
            url,
            reading_minutes,
        });
    }
    Ok(posts)
}

fn render_home(config: &SiteConfig, posts: &[RenderPost], output_dir: &Path) -> Result<()> {
    let latest = posts
        .iter()
        .take(6)
        .map(|post| render_post_card(config, post))
        .collect::<String>();
    let tag_buttons = all_tags(posts)
        .into_iter()
        .map(|tag| {
            format!(
                r#"<button class="tag" data-tag-filter="{tag}" aria-pressed="false">#{tag}</button>"#,
                tag = escape_html(&tag)
            )
        })
        .collect::<String>();

    let body = format!(
        r#"<section class="hero">
  <div class="wrap">
    <h1>{title}</h1>
    <p>{description}</p>
    <form class="search-box" action="{search_url}">
      <input type="search" name="q" placeholder="검색어" aria-label="검색어">
      <button class="button" type="submit">검색</button>
    </form>
  </div>
</section>
<section class="section">
  <div class="wrap">
    <div class="tags" aria-label="태그 필터">{tag_buttons}</div>
    <div class="grid">{latest}</div>
  </div>
</section>"#,
        title = escape_html(&config.title),
        description = escape_html(&config.description),
        search_url = with_base(config, "/search/"),
        tag_buttons = tag_buttons,
        latest = latest
    );
    write_page(output_dir.join("index.html"), &layout(config, "홈", &body))
}

fn render_post_index(config: &SiteConfig, posts: &[RenderPost], output_dir: &Path) -> Result<()> {
    let cards = posts
        .iter()
        .map(|post| render_post_card(config, post))
        .collect::<String>();
    let body = format!(
        r#"<section class="section">
  <div class="wrap">
    <h1 class="page-title">Posts</h1>
    <p class="lead">게시된 모든 글입니다.</p>
    <div class="grid">{cards}</div>
  </div>
</section>"#
    );
    write_page(
        output_dir.join("posts/index.html"),
        &layout(config, "Posts", &body),
    )
}

fn render_posts(config: &SiteConfig, posts: &[RenderPost], output_dir: &Path) -> Result<()> {
    for post in posts {
        let toc = if post.toc.is_empty() {
            String::new()
        } else {
            format!(
                r#"<aside class="toc" aria-label="Table of contents">{}</aside>"#,
                post.toc
                    .iter()
                    .filter(|heading| heading.level <= 3)
                    .map(|heading| {
                        format!(
                            r##"<a href="#{id}">{title}</a>"##,
                            id = escape_html(&heading.id),
                            title = escape_html(&heading.title)
                        )
                    })
                    .collect::<String>()
            )
        };
        let hero = post
            .fm
            .hero_image
            .as_ref()
            .map(|image| {
                format!(
                    r#"<div class="post-hero"><img src="{image}" alt="{alt}" loading="eager"></div>"#,
                    image = with_base(config, image),
                    alt = escape_html(post.fm.hero_alt.as_deref().unwrap_or(""))
                )
            })
            .unwrap_or_default();

        let body = format!(
            r#"<section class="section">
  <div class="wrap article-layout">
    <main class="article">
      <p class="meta">{date} · {minutes} min · {category}</p>
      <h1>{title}</h1>
      <p class="summary">{summary}</p>
      {hero}
      <div class="tags">{tags}</div>
      {html}
    </main>
    {toc}
  </div>
</section>"#,
            date = escape_html(&post.fm.date),
            minutes = post.reading_minutes,
            category = escape_html(&post.fm.category),
            title = escape_html(&post.fm.title),
            summary = escape_html(&post.fm.summary),
            hero = hero,
            tags = render_tags(config, &post.fm.tags),
            html = post.html,
            toc = toc
        );
        write_page(
            output_dir.join(format!("posts/{}/index.html", post.fm.slug)),
            &layout(config, &post.fm.title, &body),
        )?;
    }
    Ok(())
}

fn render_taxonomies(config: &SiteConfig, posts: &[RenderPost], output_dir: &Path) -> Result<()> {
    let mut tags: BTreeMap<String, Vec<&RenderPost>> = BTreeMap::new();
    let mut categories: BTreeMap<String, Vec<&RenderPost>> = BTreeMap::new();
    for post in posts {
        categories
            .entry(post.fm.category.clone())
            .or_default()
            .push(post);
        for tag in &post.fm.tags {
            tags.entry(tag.clone()).or_default().push(post);
        }
    }

    render_taxonomy_index(config, "Tags", "/tags/", &tags, output_dir)?;
    render_taxonomy_index(
        config,
        "Categories",
        "/categories/",
        &categories,
        output_dir,
    )?;

    for (tag, tagged_posts) in tags {
        render_taxonomy_page(
            config,
            &format!("#{tag}"),
            &tagged_posts,
            output_dir.join(format!("tags/{}/index.html", slugify(&tag))),
        )?;
    }
    for (category, category_posts) in categories {
        render_taxonomy_page(
            config,
            &category,
            &category_posts,
            output_dir.join(format!("categories/{}/index.html", slugify(&category))),
        )?;
    }
    Ok(())
}

fn render_taxonomy_index(
    config: &SiteConfig,
    title: &str,
    root: &str,
    taxonomy: &BTreeMap<String, Vec<&RenderPost>>,
    output_dir: &Path,
) -> Result<()> {
    let links = taxonomy
        .iter()
        .map(|(name, posts)| {
            format!(
                r#"<a class="tag" href="{url}">{name} ({count})</a>"#,
                url = with_base(config, &format!("{}{}/", root, slugify(name))),
                name = escape_html(name),
                count = posts.len()
            )
        })
        .collect::<String>();
    let body = format!(
        r#"<section class="section"><div class="wrap"><h1 class="page-title">{}</h1><p class="lead">분류별 글 모음입니다.</p><div class="tags">{}</div></div></section>"#,
        escape_html(title),
        links
    );
    let dir_name = root.trim_matches('/');
    write_page(
        output_dir.join(format!("{dir_name}/index.html")),
        &layout(config, title, &body),
    )
}

fn render_taxonomy_page(
    config: &SiteConfig,
    title: &str,
    posts: &[&RenderPost],
    path: PathBuf,
) -> Result<()> {
    let cards = posts
        .iter()
        .map(|post| render_post_card(config, post))
        .collect::<String>();
    let body = format!(
        r#"<section class="section"><div class="wrap"><h1 class="page-title">{}</h1><div class="grid">{}</div></div></section>"#,
        escape_html(title),
        cards
    );
    write_page(path, &layout(config, title, &body))
}

fn render_search(config: &SiteConfig, posts: &[RenderPost], output_dir: &Path) -> Result<()> {
    let items = posts
        .iter()
        .map(|post| {
            format!(
                r#"<article class="post-card" data-search-item data-post-tags="{raw_tags}" data-search-text="{search_text}">
  <div class="post-card-body">
    <p class="meta">{date} · {category}</p>
    <h2><a href="{url}">{title}</a></h2>
    <p>{summary}</p>
    <div class="tags">{tags}</div>
  </div>
</article>"#,
                raw_tags = escape_html(&post.fm.tags.join(",")),
                search_text = escape_html(&format!("{} {} {} {}", post.fm.title, post.fm.summary, post.fm.tags.join(" "), post.markdown)),
                date = escape_html(&post.fm.date),
                category = escape_html(&post.fm.category),
                url = with_base(config, &post.url),
                title = escape_html(&post.fm.title),
                summary = escape_html(&post.fm.summary),
                tags = render_tags(config, &post.fm.tags)
            )
        })
        .collect::<String>();
    let body = format!(
        r#"<section class="section">
  <div class="wrap">
    <h1 class="page-title">Search</h1>
    <div class="search-box">
      <input type="search" data-search-input placeholder="검색어" aria-label="검색어">
    </div>
    <div class="grid">{items}</div>
  </div>
</section>"#
    );
    write_page(
        output_dir.join("search/index.html"),
        &layout(config, "Search", &body),
    )
}

fn render_about(config: &SiteConfig, output_dir: &Path) -> Result<()> {
    let body = format!(
        r#"<section class="section">
  <div class="wrap article">
    <h1 class="page-title">About</h1>
    <p>{description}</p>
    <p>저작자: {author}</p>
    <p class="notice">공개 저장소 기반 워크플로우이므로 비공개 자료와 API 키를 콘텐츠 파일에 넣지 않습니다.</p>
  </div>
</section>"#,
        description = escape_html(&config.description),
        author = escape_html(&config.author)
    );
    write_page(
        output_dir.join("about/index.html"),
        &layout(config, "About", &body),
    )
}

fn render_not_found(config: &SiteConfig, output_dir: &Path) -> Result<()> {
    let body = format!(
        r#"<section class="section"><div class="wrap"><h1 class="page-title">404</h1><p class="lead">요청한 페이지를 찾을 수 없습니다.</p><p><a class="button" href="{}">홈으로</a></p></div></section>"#,
        with_base(config, "/")
    );
    write_page(output_dir.join("404.html"), &layout(config, "404", &body))
}

fn render_rss(config: &SiteConfig, posts: &[RenderPost], output_dir: &Path) -> Result<()> {
    let items = posts
        .iter()
        .take(30)
        .map(|post| {
            format!(
                r#"<item><title>{title}</title><link>{link}</link><guid>{link}</guid><pubDate>{date}</pubDate><description>{summary}</description></item>"#,
                title = escape_html(&post.fm.title),
                link = with_base(config, &post.url),
                date = escape_html(&post.fm.date),
                summary = escape_html(&post.fm.summary)
            )
        })
        .collect::<String>();
    let rss = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0"><channel><title>{title}</title><link>{link}</link><description>{description}</description>{items}</channel></rss>"#,
        title = escape_html(&config.title),
        link = with_base(config, "/"),
        description = escape_html(&config.description),
        items = items
    );
    write_page(output_dir.join("rss.xml"), &rss)
}

fn render_sitemap(config: &SiteConfig, posts: &[RenderPost], output_dir: &Path) -> Result<()> {
    let mut urls = vec![
        with_base(config, "/"),
        with_base(config, "/posts/"),
        with_base(config, "/tags/"),
        with_base(config, "/categories/"),
        with_base(config, "/about/"),
        with_base(config, "/search/"),
    ];
    urls.extend(posts.iter().map(|post| with_base(config, &post.url)));
    let entries = urls
        .into_iter()
        .map(|url| format!("<url><loc>{}</loc></url>", escape_html(&url)))
        .collect::<String>();
    let sitemap = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{entries}</urlset>"#
    );
    write_page(output_dir.join("sitemap.xml"), &sitemap)
}

fn render_search_index(posts: &[RenderPost], output_dir: &Path) -> Result<()> {
    let index = posts
        .iter()
        .map(|post| SearchItem {
            title: post.fm.title.clone(),
            slug: post.fm.slug.clone(),
            url: post.url.clone(),
            summary: post.fm.summary.clone(),
            tags: post.fm.tags.clone(),
            category: post.fm.category.clone(),
            date: post.fm.date.clone(),
            text: post.markdown.clone(),
        })
        .collect::<Vec<_>>();
    let json = serde_json::to_string_pretty(&index).context("failed to render search index")?;
    write_page(output_dir.join("search-index.json"), &json)
}

fn render_post_card(config: &SiteConfig, post: &RenderPost) -> String {
    let image = post
        .fm
        .hero_image
        .as_ref()
        .map(|image| {
            format!(
                r#"<img src="{image}" alt="{alt}" loading="lazy">"#,
                image = with_base(config, image),
                alt = escape_html(post.fm.hero_alt.as_deref().unwrap_or(""))
            )
        })
        .unwrap_or_default();
    format!(
        r#"<article class="post-card" data-post-tags="{raw_tags}">
  {image}
  <div class="post-card-body">
    <p class="meta">{date} · {category}</p>
    <h2><a href="{url}">{title}</a></h2>
    <p>{summary}</p>
    <div class="tags">{tags}</div>
  </div>
</article>"#,
        raw_tags = escape_html(&post.fm.tags.join(",")),
        image = image,
        date = escape_html(&post.fm.date),
        category = escape_html(&post.fm.category),
        url = with_base(config, &post.url),
        title = escape_html(&post.fm.title),
        summary = escape_html(&post.fm.summary),
        tags = post
            .fm
            .tags
            .iter()
            .map(|tag| format!(r#"<span class="tag">#{}</span>"#, escape_html(tag)))
            .collect::<String>()
    )
}

fn render_tags(config: &SiteConfig, tags: &[String]) -> String {
    tags.iter()
        .map(|tag| {
            format!(
                r#"<a class="tag" href="{url}">#{tag}</a>"#,
                url = with_base(config, &format!("/tags/{}/", slugify(tag))),
                tag = escape_html(tag)
            )
        })
        .collect::<String>()
}

fn layout(config: &SiteConfig, page_title: &str, body: &str) -> String {
    let title = if page_title == "홈" {
        config.title.clone()
    } else {
        format!("{page_title} · {}", config.title)
    };
    format!(
        r#"<!doctype html>
<html lang="{language}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <meta name="description" content="{description}">
  <link rel="stylesheet" href="{css}">
  <link rel="alternate" type="application/rss+xml" title="{site_title}" href="{rss}">
</head>
<body>
  <header class="site-header">
    <div class="wrap nav">
      <a class="brand" href="{home}">{site_title}</a>
      <nav class="nav-links" aria-label="Main">
        <a href="{posts}">Posts</a>
        <a href="{tags}">Tags</a>
        <a href="{categories}">Categories</a>
        <a href="{search}">Search</a>
        <a href="{about}">About</a>
      </nav>
      <button class="icon-button" type="button" data-theme-toggle aria-label="테마 전환">◐</button>
    </div>
  </header>
  {body}
  <footer class="site-footer">
    <div class="wrap">© {copyright}</div>
  </footer>
  <script src="{js}" defer></script>
</body>
</html>"#,
        language = escape_html(&config.language),
        title = escape_html(&title),
        description = escape_html(&config.description),
        css = with_base(config, "/css/site.css"),
        rss = with_base(config, "/rss.xml"),
        site_title = escape_html(&config.title),
        home = with_base(config, "/"),
        posts = with_base(config, "/posts/"),
        tags = with_base(config, "/tags/"),
        categories = with_base(config, "/categories/"),
        search = with_base(config, "/search/"),
        about = with_base(config, "/about/"),
        body = body,
        copyright = escape_html(config.copyright.as_deref().unwrap_or(&config.author)),
        js = with_base(config, "/js/openme-ui.js")
    )
}

fn prepare_headings(markdown: &str) -> (String, Vec<Heading>) {
    let mut out = String::new();
    let mut headings = Vec::new();
    let mut used_ids = BTreeSet::new();
    let mut in_code = false;

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if !in_code {
            if let Some((level, title)) = parse_heading(line) {
                let base_id = slugify(title);
                let mut id = base_id.clone();
                let mut count = 2;
                while used_ids.contains(&id) {
                    id = format!("{base_id}-{count}");
                    count += 1;
                }
                used_ids.insert(id.clone());
                headings.push(Heading {
                    level,
                    title: title.to_string(),
                    id: id.clone(),
                });
                out.push_str(&format!(
                    r#"<h{level} id="{id}">{title}</h{level}>"#,
                    level = level,
                    id = escape_html(&id),
                    title = escape_html(title)
                ));
                out.push('\n');
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    (out, headings)
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&hashes) && trimmed.chars().nth(hashes) == Some(' ') {
        Some((hashes, trimmed[hashes + 1..].trim()))
    } else {
        None
    }
}

fn all_tags(posts: &[RenderPost]) -> Vec<String> {
    posts
        .iter()
        .flat_map(|post| post.fm.tags.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn with_base(config: &SiteConfig, path: &str) -> String {
    let base = config.base_url.trim_end_matches('/');
    let path = normalize_web_path(path);
    if base.is_empty() {
        path
    } else if path == "/" {
        format!("{base}/")
    } else {
        format!("{base}{path}")
    }
}

fn write_page(path: PathBuf, content: &str) -> Result<()> {
    write_if_changed(path, content.as_bytes()).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn markdown_renderer_adds_heading_ids() {
        let (html, toc) = markdown_to_html_with_toc("## Hello Rust\n\nbody");
        assert!(html.contains(r#"<h2 id="hello-rust">Hello Rust</h2>"#));
        assert_eq!(toc[0].id, "hello-rust");
    }

    #[test]
    fn site_build_creates_index() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("content/posts")).unwrap();
        fs::create_dir_all(root.join("static/css")).unwrap();
        fs::write(root.join("static/css/site.css"), "").unwrap();
        fs::write(
            root.join("config.toml"),
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
        fs::write(
            root.join("content/posts/test.md"),
            r#"---
title: Test Post
slug: test-post
date: 2026-05-17
updated: 2026-05-17
summary: Summary
tags: [rust]
category: Test
status: published
---

# Test Post

Body."#,
        )
        .unwrap();

        let report = build_site(root.join("config.toml")).unwrap();
        assert_eq!(report.posts_rendered, 1);
        assert!(root.join("public/index.html").exists());
        assert!(root.join("public/posts/test-post/index.html").exists());
        assert!(root.join("public/search-index.json").exists());
    }
}
