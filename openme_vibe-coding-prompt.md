# vibe-coding-prompt.md
# 바이브코딩용 통합 구현 프롬프트

당신은 Rust, WebAssembly, WASI, GitHub Actions, GitHub Pages, 정적 사이트 생성기, 문서 변환기, AI 워크플로우 자동화에 능숙한 수석 소프트웨어 아키텍트이자 구현자다.

아래 요구사항을 충족하는 공개 GitHub 저장소용 프로젝트를 설계하고 구현하라.
---

## 프로젝트명

OpenMe

---

## 프로젝트 목표

GitHub Actions를 활용하여 블로그 대체 환경을 만든다.

### 필수 요구사항

1. 블로그의 주요 기능이 있는 페이지를 제공해야 한다.
2. 특정 로직을 이용한 글쓰기가 가능해야 한다.
   - 사람이 컨셉 글을 작성한다.
   - 지정된 인공지능을 통해 조사 및 분석한다.
   - 여러 AI가 각각 블로그 초안을 작성한다.
   - 초안들을 비교 평가한다.
   - 장점을 취합하고 부족한 부분을 보강하여 최종 블로그 글을 완성한다.
3. 블로그 대표 이미지를 자동 생성하고 글에 추가해야 한다.
4. 글과 이미지는 GitHub Public Repository에 저장되어야 한다.
5. GitHub Actions를 통해 GitHub Pages로 누구나 접속 가능한 웹페이지를 제공해야 한다.
6. Actions 구동을 통해 특정 폴더의 문서(`docs` 또는 `docx`)가 자동 변환되어 블로그에 게시되어야 한다.
7. 모든 사용자 정의 구현 로직은 Rust 기반 WebAssembly 방식이어야 한다.

---

## 해석 원칙

- GitHub Actions YAML, GitHub 공식 배포 액션, GitHub Pages 자체는 인프라로 간주한다.
- 커스텀 애플리케이션 로직은 Rust로 구현하고, 브라우저용 또는 WASI/Component Model용 WebAssembly로 빌드한다.
- 워크플로우 내부에서 사용자 정의 코드를 실행할 때는 Wasmtime 기반 실행을 우선한다.
- 외부 AI API 호출을 위해 `wasi:http` 또는 명확한 호스트 어댑터 계층을 고려한다.
- 최종 산출물은 GitHub Pages에서 동작하는 정적 사이트여야 한다.

---

## 구현 산출물

다음 내용을 실제 파일로 생성하라.

### 1. 저장소 구조
```text
.
├─ .github/workflows/
│  ├─ generate-post.yml
│  ├─ import-documents.yml
│  └─ deploy-pages.yml
├─ concepts/
├─ imports/docs/
├─ imports/docx/
├─ content/inbox/
├─ content/drafts/
├─ content/posts/
├─ static/images/posts/
├─ static/css/
├─ public/
├─ crates/
│  ├─ core-types/
│  ├─ doc-ingest/
│  ├─ ai-pipeline/
│  ├─ image-brief/
│  ├─ sitegen/
│  ├─ web-ui/
│  └─ common-utils/
├─ wit/
│  ├─ ai-provider.wit
│  ├─ http-client.wit
│  └─ storage.wit
├─ config/
│  ├─ site.toml
│  ├─ ai-providers.toml
│  └─ evaluation-rubric.toml
├─ Cargo.toml
├─ README.md
└─ .gitignore
```

### 2. 핵심 기능
아래를 구현하라.

#### A. sitegen
- Markdown 포스트를 HTML로 변환
- 홈, 글 목록, 단일 글 상세, 태그, 카테고리, About, 404 생성
- RSS, sitemap.xml 생성
- 검색 인덱스 JSON 생성
- `public/`에 출력

#### B. doc-ingest
- `imports/docs/*.md` 자동 처리
- front matter 보정
- `imports/docx/*.docx` 변환
- DOCX의 제목, 본문, 목록, 표, 이미지 추출을 지원하는 구조 설계
- 초기 버전에서는 완벽한 스타일 보존보다 안정적인 내용 추출을 우선
- 출력 위치는 `content/drafts/` 또는 설정에 따라 `content/posts/`

#### C. ai-pipeline
- `concepts/*.yaml` 또는 `.md` 입력
- 조사 질문 생성
- 지정 모델별 초안 생성
- 각 초안을 공통 JSON 스키마로 정규화
- 비교평가
- 보강 요청
- 최종 블로그 본문 생성
- 요약, SEO title, description, tags, slug 생성
- 최종 결과를 Markdown front matter 포함 형태로 `content/posts/`에 저장

#### D. image-brief
- 최종 글에서 대표 이미지용 프롬프트 생성
- alt text 생성
- 소셜 카드 이미지 프롬프트 생성
- 이미지 생성 API 응답을 파일로 저장하는 경로와 메타데이터 설계
- 포스트 front matter의 `hero_image`, `hero_alt` 자동 삽입

#### E. web-ui
- Rust/WASM 기반의 클라이언트 상호작용
- 검색
- 태그 필터
- 테마 전환
- 본문 목차 또는 headings 네비게이션

---

## 설정 파일

### config/site.toml
다음을 포함하라.
```toml
title = "OpenMe"
description = "GitHub Actions와 Rust/WASM 기반 자동 블로그"
base_url = "/"
language = "ko-KR"
author = "Youngsik Bae"

[build]
output_dir = "public"
posts_dir = "content/posts"
assets_dir = "static"

[publish]
default_status = "draft"
```

### config/ai-providers.toml
AI 제공자를 플러그인처럼 바꿀 수 있도록 설계하라.
```toml
[providers.researcher]
kind = "text"
model = "provider-a-model"
enabled = true

[providers.writer_a]
kind = "text"
model = "provider-b-model"
enabled = true

[providers.writer_b]
kind = "text"
model = "provider-c-model"
enabled = true

[providers.evaluator]
kind = "text"
model = "provider-d-model"
enabled = true

[providers.image]
kind = "image"
model = "provider-image-model"
enabled = true
```

### config/evaluation-rubric.toml
```toml
[weights]
accuracy = 0.25
structure = 0.15
readability = 0.15
practicality = 0.20
seo = 0.10
originality = 0.15
```

---

## 데이터 스키마

### Concept
```json
{
  "title": "string",
  "audience": "string",
  "goal": "string",
  "tone": "string",
  "length": "string",
  "keywords": ["string"],
  "references_policy": "string"
}
```

### DraftCandidate
```json
{
  "provider": "string",
  "outline": ["string"],
  "title_options": ["string"],
  "body_markdown": "string",
  "claims": ["string"],
  "limitations": ["string"]
}
```

### Evaluation
```json
{
  "candidate_id": "string",
  "scores": {
    "accuracy": 0,
    "structure": 0,
    "readability": 0,
    "practicality": 0,
    "seo": 0,
    "originality": 0
  },
  "strengths": ["string"],
  "weaknesses": ["string"],
  "recommended_revisions": ["string"]
}
```

### FinalPost
```json
{
  "title": "string",
  "slug": "string",
  "summary": "string",
  "tags": ["string"],
  "category": "string",
  "hero_image_prompt": "string",
  "hero_alt": "string",
  "body_markdown": "string"
}
```

---

## GitHub Actions

### generate-post.yml
- 트리거:
  - `concepts/**`
  - `workflow_dispatch`
- 실행:
  - Rust/WASM 빌드
  - Wasmtime 설치
  - AI pipeline WASM 실행
  - 이미지 생성 단계 실행
  - 결과 파일 커밋
  - 이어서 배포 워크플로우가 동작 가능하도록 구성

### import-documents.yml
- 트리거:
  - `imports/docs/**`
  - `imports/docx/**`
- 실행:
  - Rust/WASM 문서 변환기 실행
  - draft/post 생성
  - 변경사항 커밋

### deploy-pages.yml
- 트리거:
  - `content/posts/**`
  - `static/images/**`
  - `workflow_dispatch`
- 실행:
  - 정적 사이트 생성기 WASM 실행
  - `public/` 결과 생성
  - GitHub Pages 배포

---

## 보안 요구사항

- API 키는 절대 코드에 하드코딩하지 말 것
- GitHub Actions Secret 사용
- 커밋 권한은 필요한 워크플로우에만 최소 부여
- 공개 저장소에 민감정보가 들어가지 않도록 README에 경고 포함
- 실패 시 로그에 Secret이 노출되지 않도록 마스킹 고려

---

## 테스트 요구사항

### Unit Tests
- slug 생성
- front matter 렌더링
- DOCX 변환 결과 일부
- 평가 점수 계산
- Markdown → HTML 변환

### Integration Tests
- 샘플 concept → 최종 post 생성
- 샘플 markdown import → post 생성
- 샘플 docx import → draft 생성
- sitegen → public/index.html 존재

### Snapshot Tests
- 렌더링된 HTML
- 생성된 RSS
- 생성된 sitemap

---

## 구현 우선순위

1. 프로젝트 스캐폴딩
2. sitegen
3. GitHub Pages 배포
4. docs Markdown import
5. DOCX draft import
6. 단일 AI post pipeline
7. 다중 AI 비교평가
8. 대표 이미지 자동 생성
9. WASI/Component Model 기반 HTTP 고도화
10. 사용자 경험 개선

---

## 개발 스타일

- 모든 코드는 명확하고 유지보수성이 높아야 한다.
- 불필요한 추상화를 피하라.
- 핵심 도메인 타입은 `core-types`에 모아라.
- 모듈 간 인터페이스는 JSON Schema 또는 Rust 타입으로 명확히 하라.
- IO와 순수 로직을 분리하라.
- 실패 가능한 경로는 `Result`로 처리하라.
- Actions 단계는 재실행 가능하고 멱등적이어야 한다.
- 생성 결과가 동일하면 중복 커밋하지 않도록 하라.

---

## 최종 응답 형식

작업을 다음 순서로 진행하라.

1. 전체 설계 개요
2. 저장소 파일 트리
3. 각 crate의 역할
4. 주요 Rust 타입
5. 각 GitHub Actions workflow YAML
6. sitegen의 최소 구현 코드
7. doc-ingest의 최소 구현 코드
8. ai-pipeline의 인터페이스 코드
9. image-brief의 인터페이스 코드
10. README.md
11. 남은 리스크와 다음 단계

가능한 한 실제로 작동 가능한 코드에 가깝게 작성하라.
