use anyhow::{Context, Result};
use common_utils::{
    list_files_with_exts, markdown_excerpt, markdown_title, parse_front_matter, read_toml_file,
    read_yaml_file, render_markdown_document, slugify, today_string, write_if_changed,
};
use core_types::{
    AiProvidersConfig, Concept, ContentSource, DraftCandidate, Evaluation, EvaluationScores,
    FinalPost, PipelineReport, PostFrontMatter, ProviderConfig, RubricConfig, SiteConfig,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Default)]
struct PartialConcept {
    title: Option<String>,
    audience: Option<String>,
    goal: Option<String>,
    tone: Option<String>,
    length: Option<String>,
    keywords: Option<Vec<String>>,
    references_policy: Option<String>,
    references: Option<core_types::ConceptReferences>,
}

pub fn run_pipeline(
    concepts_dir: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
    providers_path: impl AsRef<Path>,
    rubric_path: impl AsRef<Path>,
    site_config_path: impl AsRef<Path>,
) -> Result<Vec<PipelineReport>> {
    let providers: AiProvidersConfig = read_toml_file(providers_path)?;
    let rubric: RubricConfig = read_toml_file(rubric_path)?;
    let site_config: SiteConfig = read_toml_file(site_config_path)?;
    let mut reports = Vec::new();

    let mut concept_files = list_files_with_exts(&concepts_dir, &["yaml", "yml"])?;
    concept_files.extend(list_files_with_exts(&concepts_dir, &["md"])?);
    concept_files.sort();

    for path in concept_files {
        let concept = read_concept(&path)?;
        let report = process_concept(
            &concept,
            &path,
            output_dir.as_ref(),
            &providers,
            &rubric,
            &site_config,
        )?;
        reports.push(report);
    }
    Ok(reports)
}

pub fn process_concept(
    concept: &Concept,
    concept_path: &Path,
    output_dir: &Path,
    providers: &AiProvidersConfig,
    rubric: &RubricConfig,
    site_config: &SiteConfig,
) -> Result<PipelineReport> {
    let slug = slugify(&concept.title);
    let research_questions = research_questions(concept);
    let writers = writer_providers(providers);
    let candidates = writers
        .iter()
        .map(|(name, provider)| {
            load_or_create_candidate(&slug, name, provider, concept, &research_questions)
        })
        .collect::<Result<Vec<_>>>()?;
    let candidates = if candidates.is_empty() {
        vec![create_candidate(
            "local_writer",
            &ProviderConfig {
                kind: "text".to_string(),
                model: "deterministic-local-scaffold".to_string(),
                enabled: true,
                endpoint_env: None,
            },
            concept,
            &research_questions,
        )]
    } else {
        candidates
    };
    let evaluations = candidates
        .iter()
        .map(|candidate| evaluate_candidate(candidate, concept, &rubric.weights))
        .collect::<Vec<_>>();
    let final_post = synthesize_final_post(
        concept,
        &candidates,
        &evaluations,
        &rubric.weights,
        &site_config.author,
    );
    let front_matter = final_post_front_matter(&final_post, concept_path);
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let output = output_dir.join(format!("{}.md", final_post.slug));
    let markdown = render_markdown_document(&front_matter, &final_post.body_markdown)?;
    write_if_changed(&output, markdown.as_bytes())?;

    let report = PipelineReport {
        concept: concept_path.to_string_lossy().to_string(),
        slug,
        research_questions,
        candidates,
        evaluations,
        output: output.to_string_lossy().to_string(),
    };
    let report_dir = PathBuf::from("content/drafts/ai-reports");
    let report_path = report_dir.join(format!("{}.pipeline.json", report.slug));
    let report_json = serde_json::to_string_pretty(&report)?;
    write_if_changed(report_path, report_json.as_bytes())?;
    Ok(report)
}

fn read_concept(path: &Path) -> Result<Concept> {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
    {
        "yaml" | "yml" => read_yaml_file(path),
        "md" => {
            let raw = fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let (front, body): (Option<PartialConcept>, String) = parse_front_matter(&raw)?;
            let partial = front.unwrap_or_default();
            let title = partial
                .title
                .or_else(|| markdown_title(&body))
                .or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "Untitled concept".to_string());
            Ok(Concept {
                title,
                audience: partial.audience.unwrap_or_else(|| "기술 독자".to_string()),
                goal: partial.goal.unwrap_or_else(|| markdown_excerpt(&body, 180)),
                tone: partial
                    .tone
                    .unwrap_or_else(|| "명확하고 실무적인 설명".to_string()),
                length: partial.length.unwrap_or_else(|| "2500-4000자".to_string()),
                keywords: partial.keywords.unwrap_or_default(),
                references_policy: partial.references_policy,
                references: partial.references,
            })
        }
        _ => anyhow::bail!("unsupported concept file: {}", path.display()),
    }
}

pub fn research_questions(concept: &Concept) -> Vec<String> {
    let mut questions = vec![
        format!(
            "{} 주제를 구현할 때 반드시 확인해야 할 공식 제약은 무엇인가?",
            concept.title
        ),
        format!(
            "대상 독자({})가 실제로 겪는 문제와 성공 기준은 무엇인가?",
            empty_fallback(&concept.audience, "기술 독자")
        ),
        "GitHub Actions, Pages, Rust/WASM 구성에서 실패하기 쉬운 운영 지점은 무엇인가?".to_string(),
        "최종 글에서 단정적으로 쓰면 안 되는 주장과 검증이 필요한 항목은 무엇인가?".to_string(),
    ];
    for keyword in &concept.keywords {
        questions.push(format!(
            "{keyword}와 관련해 독자가 바로 적용할 수 있는 실무 체크포인트는 무엇인가?"
        ));
    }
    questions
}

fn writer_providers(providers: &AiProvidersConfig) -> Vec<(String, ProviderConfig)> {
    providers
        .providers
        .iter()
        .filter(|(name, provider)| {
            provider.enabled && provider.kind == "text" && name.starts_with("writer")
        })
        .map(|(name, provider)| (name.clone(), provider.clone()))
        .collect()
}

fn load_or_create_candidate(
    slug: &str,
    provider_name: &str,
    provider: &ProviderConfig,
    concept: &Concept,
    research_questions: &[String],
) -> Result<DraftCandidate> {
    if let Ok(dir) = std::env::var("OPENME_AI_RESPONSE_DIR") {
        let path = PathBuf::from(dir).join(format!("{slug}-{provider_name}.json"));
        if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let mut candidate: DraftCandidate = serde_json::from_str(&raw)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            candidate.provider = provider_name.to_string();
            return Ok(candidate);
        }
    }
    Ok(create_candidate(
        provider_name,
        provider,
        concept,
        research_questions,
    ))
}

fn create_candidate(
    provider_name: &str,
    provider: &ProviderConfig,
    concept: &Concept,
    research_questions: &[String],
) -> DraftCandidate {
    let keywords = if concept.keywords.is_empty() {
        "Rust, WebAssembly, GitHub Actions".to_string()
    } else {
        concept.keywords.join(", ")
    };
    let outline = vec![
        "문제 정의와 독자 맥락".to_string(),
        "GitHub Actions 기반 자동화 구조".to_string(),
        "Rust/WASM 모듈별 책임".to_string(),
        "AI 글쓰기와 이미지 생성 파이프라인".to_string(),
        "운영 보안과 다음 개선".to_string(),
    ];
    let body_markdown = format!(
        r#"## 문제 정의

{title}는 단순한 정적 블로그 구축이 아니라, 컨셉 입력부터 게시까지 반복 가능한 운영 흐름을 만드는 문제다. 대상 독자는 {audience}이며, 글의 목표는 {goal}이다.

## 구현 방향

핵심 로직은 Rust crate로 분리하고, GitHub Actions에서는 WASI 모듈을 Wasmtime으로 실행한다. Markdown과 DOCX 입력은 문서 변환 단계에서 표준 front matter를 가진 글로 정규화한다. 컨셉 입력은 조사 질문, 모델별 초안, 비교평가, 통합 원고 순서로 처리한다.

## AI 파이프라인 초안

{provider_name}({model}) 관점에서는 {keywords}를 중심으로 구조를 잡는다. 조사 질문은 다음 항목을 우선 확인한다.

{questions}

## 운영 체크포인트

API 키는 GitHub Actions Secret으로만 전달하고, 공개 저장소에는 최종 게시 가능한 글과 이미지 자산만 남긴다. 생성 결과가 동일하면 커밋하지 않도록 파일 쓰기를 멱등적으로 처리한다.
"#,
        title = concept.title,
        audience = empty_fallback(&concept.audience, "개발자"),
        goal = empty_fallback(&concept.goal, "구현 방향 제시"),
        provider_name = provider_name,
        model = provider.model,
        keywords = keywords,
        questions = research_questions
            .iter()
            .map(|question| format!("- {question}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    DraftCandidate {
        provider: provider_name.to_string(),
        outline,
        title_options: vec![
            concept.title.clone(),
            format!("{}: Rust/WASM 자동화 설계", concept.title),
            "GitHub Actions로 운영하는 WASM 블로그 파이프라인".to_string(),
        ],
        body_markdown,
        claims: vec![
            "커스텀 로직을 Rust/WASM으로 분리하면 Actions 실행 경로를 재현하기 쉽다.".to_string(),
            "정적 사이트 결과물은 GitHub Pages와 잘 맞는다.".to_string(),
        ],
        limitations: vec![
            "실제 AI API 호출은 provider별 host adapter 또는 wasi:http 연결이 필요하다."
                .to_string(),
            "DOCX의 복잡한 스타일 보존은 단계적 개선 대상이다.".to_string(),
        ],
    }
}

pub fn evaluate_candidate(
    candidate: &DraftCandidate,
    concept: &Concept,
    weights: &BTreeMap<String, f32>,
) -> Evaluation {
    let body = candidate.body_markdown.to_lowercase();
    let keyword_hits = concept
        .keywords
        .iter()
        .filter(|keyword| body.contains(&keyword.to_lowercase()))
        .count();
    let keyword_ratio = if concept.keywords.is_empty() {
        1.0
    } else {
        keyword_hits as f32 / concept.keywords.len() as f32
    };
    let outline_score = (candidate.outline.len() as f32 / 5.0).min(1.0);
    let length_score = (candidate.body_markdown.chars().count() as f32 / 1800.0).min(1.0);
    let limitation_penalty = (candidate.limitations.len() as f32 * 0.15).min(0.7);
    let scores = EvaluationScores {
        accuracy: (7.0 + keyword_ratio * 2.0 - limitation_penalty).clamp(0.0, 10.0),
        structure: (6.5 + outline_score * 3.0).clamp(0.0, 10.0),
        readability: (7.0 + length_score).clamp(0.0, 10.0),
        practicality: (7.0 + body.matches("운영").count() as f32 * 0.25).clamp(0.0, 10.0),
        seo: (6.5 + keyword_ratio * 2.5).clamp(0.0, 10.0),
        originality: (7.0 + candidate.claims.len() as f32 * 0.25).clamp(0.0, 10.0),
    };
    let total = scores.weighted_total(weights);
    Evaluation {
        candidate_id: candidate.provider.clone(),
        scores,
        strengths: vec![
            format!("가중 총점 {:.2}점", total),
            "요구사항을 단계별 운영 흐름으로 설명함".to_string(),
        ],
        weaknesses: candidate.limitations.clone(),
        recommended_revisions: vec![
            "외부 API 어댑터와 Secret 운영 정책을 명확히 유지".to_string(),
            "DOCX 변환 한계를 README와 리스크 항목에 표시".to_string(),
        ],
    }
}

fn synthesize_final_post(
    concept: &Concept,
    candidates: &[DraftCandidate],
    evaluations: &[Evaluation],
    weights: &BTreeMap<String, f32>,
    author: &str,
) -> FinalPost {
    let best = evaluations
        .iter()
        .max_by(|a, b| {
            a.scores
                .weighted_total(weights)
                .partial_cmp(&b.scores.weighted_total(weights))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|evaluation| {
            candidates
                .iter()
                .find(|candidate| candidate.provider == evaluation.candidate_id)
        })
        .unwrap_or(&candidates[0]);

    let tags = if concept.keywords.is_empty() {
        vec![
            "github-actions".to_string(),
            "rust".to_string(),
            "wasm".to_string(),
        ]
    } else {
        concept
            .keywords
            .iter()
            .map(|keyword| slugify(keyword))
            .collect()
    };
    let slug = slugify(&concept.title);
    let summary = format!(
        "{}를 GitHub Actions, Rust/WASM, GitHub Pages 기반 자동화 흐름으로 구현하는 방법을 정리합니다.",
        concept.title
    );
    let body = format!(
        r#"# {title}

{summary}

## 컨셉과 성공 기준

이 글은 {audience}를 대상으로 한다. 목표는 {goal}이며, 문체는 {tone}에 맞춘다. 참고 정책은 "{references_policy}"를 기본으로 한다.

## 조사 질문

{questions}

## 권장 아키텍처

입력 계층은 `concepts/`, `imports/docs/`, `imports/docx/`로 나눈다. 변환 계층은 `doc-ingest.wasm`, 글쓰기 계층은 `ai-pipeline.wasm`, 이미지 계층은 `image-brief.wasm`, 사이트 생성 계층은 `sitegen.wasm`이 담당한다. GitHub Actions는 이 WASM 모듈을 Wasmtime으로 실행하고 결과를 저장소에 커밋한다.

## 초안 비교 결과

{evaluation_summary}

## 통합 원고

{best_body}

## 대표 이미지 정책

최종 글이 생성되면 `image-brief`가 대표 이미지 프롬프트와 alt text를 만든다. 외부 이미지 API를 연결한 환경에서는 응답 파일을 `static/images/posts/{slug}.webp`와 `static/images/posts/{slug}-social.webp`로 저장하고, API가 연결되지 않은 로컬 또는 CI 검증 환경에서는 결정론적 WebP 플레이스홀더를 생성한다.

## 보안과 운영

API 키는 코드에 하드코딩하지 않는다. GitHub Actions Secret을 통해 provider host adapter에만 주입하고, 로그에는 민감값이 찍히지 않도록 한다. 공개 저장소에는 컨셉, 초안, 최종 글, 이미지가 남을 수 있으므로 비공개 자료는 입력하지 않는다.

## 저작자

{author}
"#,
        title = concept.title,
        summary = summary,
        audience = empty_fallback(&concept.audience, "기술 독자"),
        goal = empty_fallback(&concept.goal, "구현 방향 제시"),
        tone = empty_fallback(&concept.tone, "명확하고 실무적인 설명"),
        references_policy = concept.reference_policy(),
        questions = research_questions(concept)
            .into_iter()
            .map(|question| format!("- {question}"))
            .collect::<Vec<_>>()
            .join("\n"),
        evaluation_summary = evaluations
            .iter()
            .map(|evaluation| {
                format!(
                    "- `{}`: accuracy {:.1}, structure {:.1}, practicality {:.1}",
                    evaluation.candidate_id,
                    evaluation.scores.accuracy,
                    evaluation.scores.structure,
                    evaluation.scores.practicality
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        best_body = best.body_markdown,
        slug = slug,
        author = author
    );

    FinalPost {
        title: concept.title.clone(),
        slug: slug.clone(),
        summary,
        tags,
        category: "Automation".to_string(),
        hero_image_prompt: format!(
            "16:9 editorial illustration for a Korean technical blog about {}, Rust, WebAssembly, GitHub Actions workflow, clean interface, no readable text",
            concept.title
        ),
        hero_alt: format!("{} 자동화 흐름을 표현한 대표 이미지", concept.title),
        body_markdown: body,
    }
}

fn final_post_front_matter(final_post: &FinalPost, concept_path: &Path) -> PostFrontMatter {
    let date = today_string();
    PostFrontMatter {
        title: final_post.title.clone(),
        slug: final_post.slug.clone(),
        date: date.clone(),
        updated: date,
        summary: final_post.summary.clone(),
        tags: final_post.tags.clone(),
        category: final_post.category.clone(),
        hero_image: None,
        hero_image_prompt: Some(final_post.hero_image_prompt.clone()),
        hero_alt: Some(final_post.hero_alt.clone()),
        status: "published".to_string(),
        source: Some(ContentSource {
            kind: "concept".to_string(),
            path: concept_path.to_string_lossy().to_string(),
        }),
    }
}

fn empty_fallback<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_question_generation_uses_keywords() {
        let concept = Concept {
            title: "테스트".to_string(),
            audience: "개발자".to_string(),
            goal: "검증".to_string(),
            tone: "명확".to_string(),
            length: "짧게".to_string(),
            keywords: vec!["rust wasm".to_string()],
            references_policy: None,
            references: None,
        };
        let questions = research_questions(&concept);
        assert!(questions
            .iter()
            .any(|question| question.contains("rust wasm")));
    }

    #[test]
    fn evaluation_scores_keyword_coverage() {
        let concept = Concept {
            title: "테스트".to_string(),
            audience: String::new(),
            goal: String::new(),
            tone: String::new(),
            length: String::new(),
            keywords: vec!["rust".to_string()],
            references_policy: None,
            references: None,
        };
        let candidate = DraftCandidate {
            provider: "writer".to_string(),
            outline: vec!["a".to_string(); 5],
            title_options: vec![],
            body_markdown: "rust 운영".to_string(),
            claims: vec!["claim".to_string()],
            limitations: vec![],
        };
        let evaluation = evaluate_candidate(&candidate, &concept, &BTreeMap::new());
        assert!(evaluation.scores.accuracy >= 9.0);
    }
}
