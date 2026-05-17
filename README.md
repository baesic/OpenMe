# OpenMe

저작자: Whoborn Inc. baesic

OpenMe는 GitHub Public Repository, GitHub Actions, GitHub Pages를 기반으로 블로그 게시를 자동화하는 Rust/WebAssembly 프로젝트입니다. 사용자 정의 로직은 Rust crate로 구현하고, Actions에서는 `wasm32-wasip1` 빌드 결과를 Wasmtime으로 실행합니다. 브라우저 검색과 필터링 점수 계산은 `wasm32-unknown-unknown`으로 빌드한 Rust/WASM 모듈을 사용합니다.

## 1. 전체 설계 개요

입력은 `concepts/*.yaml|md`, `imports/docs/*.md`, `imports/docx/*.docx`입니다. `doc-ingest`는 문서를 표준 Markdown post로 정규화하고, `ai-pipeline`은 컨셉에서 조사 질문, 다중 초안, 평가, 최종 포스트를 생성합니다. `image-brief`는 대표 이미지 프롬프트와 alt text를 만들고 이미지 파일 및 메타데이터를 저장합니다. `sitegen`은 `content/posts/*.md`를 GitHub Pages용 정적 사이트로 렌더링합니다.

외부 AI/API 호출은 `wit/ai-provider.wit`, `wit/http-client.wit`의 host adapter 계층으로 분리합니다. 현재 구현은 API가 없는 환경에서도 재현 가능한 결정론적 초안과 WebP 이미지를 생성하며, `OPENME_AI_RESPONSE_DIR`, `OPENME_IMAGE_RESPONSE_DIR`을 사용하면 외부 provider 응답 파일을 주입할 수 있습니다.

## 2. 저장소 파일 트리

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
├─ static/js/
├─ public/
├─ crates/
│  ├─ core-types/
│  ├─ common-utils/
│  ├─ doc-ingest/
│  ├─ ai-pipeline/
│  ├─ image-brief/
│  ├─ sitegen/
│  └─ web-ui/
├─ wit/
├─ config/
├─ scripts/
├─ Cargo.toml
└─ README.md
```

## 3. 각 crate의 역할

- `core-types`: Concept, DraftCandidate, Evaluation, FinalPost, PostFrontMatter, SiteConfig 등 공용 타입.
- `common-utils`: front matter 파싱/렌더링, slug, 날짜, 파일 쓰기, 디렉터리 복사.
- `doc-ingest`: Markdown front matter 보정, DOCX ZIP/XML 파싱, 표/목록/이미지 추출 구조.
- `ai-pipeline`: 컨셉 분석, 조사 질문 생성, provider별 초안 정규화, 루브릭 평가, 최종 Markdown 생성.
- `image-brief`: 대표 이미지 프롬프트/alt/social prompt 생성, WebP 이미지와 JSON 메타데이터 저장, front matter 갱신.
- `sitegen`: 홈, 목록, 상세, 태그, 카테고리, About, Search, 404, RSS, sitemap, 검색 인덱스 생성.
- `web-ui`: 검색 점수 계산을 Rust/WASM으로 제공하고 JS host adapter가 DOM 검색, 태그 필터, 테마 전환을 연결.

## 4. 주요 Rust 타입

핵심 타입은 `crates/core-types/src/lib.rs`에 있습니다.

- `Concept`: 사람이 작성한 컨셉 입력.
- `DraftCandidate`: provider별 블로그 초안.
- `Evaluation`: 공통 루브릭 기반 평가 결과.
- `FinalPost`: 최종 글과 SEO/이미지 메타데이터.
- `PostFrontMatter`: 게시용 Markdown front matter.
- `ImageBrief`: 대표 이미지, 소셜 카드 이미지, alt text, prompt.

## 5. GitHub Actions workflow

- `generate-post.yml`: `concepts/**` 변경 또는 수동 실행 시 `ai-pipeline.wasm`과 `image-brief.wasm`을 실행하고 결과를 커밋합니다.
- `import-documents.yml`: `imports/docs/**`, `imports/docx/**` 변경 시 `doc-ingest.wasm`을 실행하고 draft/post를 커밋합니다.
- `deploy-pages.yml`: `content/posts/**`, `static/**` 변경 시 `sitegen.wasm`으로 `public/`을 생성하고 GitHub Pages에 배포합니다.

## 6. sitegen 최소 구현

`sitegen`은 Markdown front matter를 읽고 `pulldown-cmark`로 HTML을 생성합니다. 생성물은 `public/` 아래에 배치됩니다.

```sh
cargo build --release --target wasm32-wasip1 -p sitegen
wasmtime --dir . target/wasm32-wasip1/release/sitegen.wasm -- --config config/site.toml
```

생성 페이지:

- `/index.html`
- `/posts/index.html`
- `/posts/{slug}/index.html`
- `/tags/`, `/tags/{tag}/`
- `/categories/`, `/categories/{category}/`
- `/about/`
- `/search/`
- `/404.html`
- `/rss.xml`
- `/sitemap.xml`
- `/search-index.json`

## 7. doc-ingest 최소 구현

Markdown은 front matter가 없거나 부족하면 제목, slug, 날짜, 요약, 상태를 보정합니다. DOCX는 ZIP 내부의 `word/document.xml`을 `quick-xml`로 읽어 제목, 단락, 목록, 표를 Markdown으로 변환하고 `word/media/*` 이미지를 `static/images/imported/`로 추출합니다.

```sh
cargo build --release --target wasm32-wasip1 -p doc-ingest
wasmtime --dir . target/wasm32-wasip1/release/doc-ingest.wasm -- \
  --config config/site.toml \
  --docs imports/docs \
  --docx imports/docx
```

## 8. ai-pipeline 인터페이스

`config/ai-providers.toml`의 enabled provider 중 `writer_*`가 초안 후보가 됩니다. 실제 API가 연결되지 않은 환경에서는 결정론적 초안을 만들고, `OPENME_AI_RESPONSE_DIR/{slug}-{provider}.json` 파일이 있으면 해당 JSON을 `DraftCandidate`로 읽습니다.

```sh
cargo build --release --target wasm32-wasip1 -p ai-pipeline
wasmtime --dir . target/wasm32-wasip1/release/ai-pipeline.wasm -- \
  --concepts concepts \
  --output content/posts \
  --providers config/ai-providers.toml \
  --rubric config/evaluation-rubric.toml
```

## 9. image-brief 인터페이스

`image-brief`는 최종 글에서 대표 이미지 prompt, social card prompt, alt text를 생성합니다. `OPENME_IMAGE_RESPONSE_DIR`에 외부 이미지 API 응답 파일이 있으면 복사하고, 없으면 검증 가능한 WebP 이미지를 결정론적으로 생성합니다.

```sh
cargo build --release --target wasm32-wasip1 -p image-brief
wasmtime --dir . target/wasm32-wasip1/release/image-brief.wasm -- \
  --posts content/posts \
  --images static/images/posts
```

## 10. 로컬 실행

```sh
rustup target add wasm32-wasip1 wasm32-unknown-unknown
cargo test --workspace
cargo build --release --target wasm32-wasip1 -p doc-ingest -p ai-pipeline -p image-brief -p sitegen
cargo build --release --target wasm32-unknown-unknown -p web-ui
wasmtime --dir . target/wasm32-wasip1/release/doc-ingest.wasm -- --config config/site.toml
wasmtime --dir . target/wasm32-wasip1/release/ai-pipeline.wasm -- --concepts concepts --output content/posts
wasmtime --dir . target/wasm32-wasip1/release/image-brief.wasm -- --posts content/posts --images static/images/posts
wasmtime --dir . target/wasm32-wasip1/release/sitegen.wasm -- --config config/site.toml
mkdir -p public/wasm
cp target/wasm32-unknown-unknown/release/openme_web_ui.wasm public/wasm/openme_web_ui.wasm
```

## 11. 남은 리스크와 다음 단계

- 실제 AI provider 호출은 `wasi:http` 또는 host wrapper 구현이 필요합니다.
- DOCX의 각주, 주석, 복잡한 스타일, 이미지 위치 보존은 2차 개선 범위입니다.
- 공개 저장소에 컨셉, 초안, 이미지가 남을 수 있으므로 비공개 자료를 입력하지 않아야 합니다.
- GitHub Pages에서 repository subpath를 쓰는 경우 `config/site.toml`의 `base_url`을 저장소 경로로 맞춰야 합니다.

## 보안

API 키를 코드, config, Markdown, concept 파일에 저장하지 마세요. GitHub Actions Secret 또는 외부 host adapter의 비밀 저장소를 사용해야 합니다. 로그에는 Secret 값이 노출되지 않도록 provider wrapper에서 마스킹을 적용하세요.
