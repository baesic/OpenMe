use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SiteConfig {
    pub title: String,
    pub description: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_language")]
    pub language: String,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub publish: PublishConfig,
    #[serde(default)]
    pub docx: DocxConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildConfig {
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    #[serde(default = "default_posts_dir")]
    pub posts_dir: String,
    #[serde(default = "default_assets_dir")]
    pub assets_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublishConfig {
    #[serde(default = "default_status")]
    pub default_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocxConfig {
    #[serde(default = "default_docx_publish_mode")]
    pub publish_mode: String,
    #[serde(default = "default_true")]
    pub extract_images: bool,
    #[serde(default = "default_true")]
    pub convert_tables: bool,
    #[serde(default = "default_true")]
    pub preserve_headings: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            posts_dir: default_posts_dir(),
            assets_dir: default_assets_dir(),
        }
    }
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self {
            default_status: default_status(),
        }
    }
}

impl Default for DocxConfig {
    fn default() -> Self {
        Self {
            publish_mode: default_docx_publish_mode(),
            extract_images: true,
            convert_tables: true,
            preserve_headings: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiProvidersConfig {
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    pub kind: String,
    pub model: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RubricConfig {
    #[serde(default)]
    pub weights: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Concept {
    pub title: String,
    #[serde(default)]
    pub audience: String,
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub tone: String,
    #[serde(default)]
    pub length: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub references_policy: Option<String>,
    #[serde(default)]
    pub references: Option<ConceptReferences>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConceptReferences {
    #[serde(default)]
    pub policy: String,
}

impl Concept {
    pub fn reference_policy(&self) -> String {
        self.references_policy
            .clone()
            .or_else(|| self.references.as_ref().map(|r| r.policy.clone()))
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "공식 문서와 공개 가능한 근거 우선".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DraftCandidate {
    pub provider: String,
    #[serde(default)]
    pub outline: Vec<String>,
    #[serde(default)]
    pub title_options: Vec<String>,
    pub body_markdown: String,
    #[serde(default)]
    pub claims: Vec<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Evaluation {
    pub candidate_id: String,
    pub scores: EvaluationScores,
    #[serde(default)]
    pub strengths: Vec<String>,
    #[serde(default)]
    pub weaknesses: Vec<String>,
    #[serde(default)]
    pub recommended_revisions: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct EvaluationScores {
    pub accuracy: f32,
    pub structure: f32,
    pub readability: f32,
    pub practicality: f32,
    pub seo: f32,
    pub originality: f32,
}

impl EvaluationScores {
    pub fn weighted_total(&self, weights: &BTreeMap<String, f32>) -> f32 {
        self.accuracy * weight(weights, "accuracy")
            + self.structure * weight(weights, "structure")
            + self.readability * weight(weights, "readability")
            + self.practicality * weight(weights, "practicality")
            + self.seo * weight(weights, "seo")
            + self.originality * weight(weights, "originality")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinalPost {
    pub title: String,
    pub slug: String,
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub category: String,
    pub hero_image_prompt: String,
    pub hero_alt: String,
    pub body_markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PostFrontMatter {
    pub title: String,
    pub slug: String,
    pub date: String,
    pub updated: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero_image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero_image_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero_alt: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ContentSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContentSource {
    #[serde(rename = "type")]
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageBrief {
    pub slug: String,
    pub hero_image_prompt: String,
    pub hero_alt: String,
    pub social_card_prompt: String,
    pub hero_image: String,
    pub social_image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchItem {
    pub title: String,
    pub slug: String,
    pub url: String,
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub category: String,
    pub date: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocIngestReport {
    pub scanned: usize,
    pub written: usize,
    #[serde(default)]
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PipelineReport {
    pub concept: String,
    pub slug: String,
    #[serde(default)]
    pub research_questions: Vec<String>,
    #[serde(default)]
    pub candidates: Vec<DraftCandidate>,
    #[serde(default)]
    pub evaluations: Vec<Evaluation>,
    pub output: String,
}

fn weight(weights: &BTreeMap<String, f32>, key: &str) -> f32 {
    weights.get(key).copied().unwrap_or(1.0 / 6.0)
}

fn default_base_url() -> String {
    "/".to_string()
}

fn default_language() -> String {
    "ko-KR".to_string()
}

fn default_output_dir() -> String {
    "public".to_string()
}

fn default_posts_dir() -> String {
    "content/posts".to_string()
}

fn default_assets_dir() -> String {
    "static".to_string()
}

fn default_status() -> String {
    "draft".to_string()
}

fn default_docx_publish_mode() -> String {
    "draft".to_string()
}

fn default_true() -> bool {
    true
}
