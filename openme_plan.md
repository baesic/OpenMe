# plan.md
# GitHub Actions 기반 Rust/WASM 블로그 자동화 플랫폼 기획서

- 문서 버전: v1.0
- 작성일: 2026-05-17
- 목표: GitHub Public Repository와 GitHub Actions, GitHub Pages를 활용하여
  1) 블로그 사이트를 제공하고,
  2) 컨셉 입력 → 다중 AI 조사·분석·집필 → 비교평가·보강 → 최종 포스트 생성,
  3) 대표 이미지 자동 생성,
  4) 문서 폴더의 Markdown/DOCX 자동 게시,
  5) 모든 사용자 정의 구현 로직을 Rust 기반 WebAssembly로 구성하는 시스템을 설계한다.

---

## 1. 결론

### 1.1 구현 가능성
본 프로젝트는 구현 가능하다.

다만 “모든 구현은 Rust 기반 WASM”이라는 요구사항을 엄격히 적용하면 다음과 같이 해석해야 한다.

- GitHub Actions의 YAML, GitHub 공식 배포 Action, GitHub Pages 자체는 인프라 계층이므로 제외
- 사용자가 직접 작성하는 핵심 로직은 Rust로 구현하고 WebAssembly/WASI 대상으로 컴파일
- GitHub Actions에서는 Wasmtime을 통해 `.wasm` 또는 컴포넌트 `.wasm`을 실행
- 브라우저 측 상호작용은 Rust → `wasm32-unknown-unknown` 빌드
- 배치 파이프라인은 Rust → WASI/Component Model 빌드

이 접근은 가능하지만 일반적인 네이티브 Rust CLI보다 난도가 높다.

### 1.2 권장 개발 원칙
본 기획서는 다음 원칙을 따른다.

1. **사이트 결과물은 정적 웹사이트**
   - GitHub Pages에 최적화
   - HTML/CSS/JS/WASM/이미지 파일을 배포

2. **콘텐츠 생성은 Actions에서 수행**
   - 컨셉 파일 또는 문서 파일 변경 감지
   - AI 파이프라인 자동 실행
   - 생성 결과를 저장소에 커밋
   - 사이트 빌드 후 Pages에 배포

3. **커스텀 로직은 Rust/WASM**
   - 문서 정규화
   - DOCX 파싱/Markdown 변환
   - AI 프롬프트 조합
   - AI 응답 비교평가
   - 최종 블로그 글 합성
   - 이미지 프롬프트 생성
   - 사이트 정적 파일 생성

4. **AI 제공자는 어댑터화**
   - 특정 LLM이나 이미지 생성 모델에 종속되지 않도록 설계
   - OpenAI, Anthropic, Google, 로컬 모델 API 등으로 교체 가능하게 함

---

## 2. 요구사항별 구현성 분석

| 번호 | 요구사항 | 구현 가능성 | 판단 |
|---|---|---:|---|
| 1 | 블로그 주요 기능 페이지 제공 | 가능 | 정적 페이지 생성기로 구현 가능 |
| 2 | 컨셉 입력 → 조사·분석 → 다중 AI 초안 → 비교평가 → 보강 → 최종 글 작성 | 가능 | Actions 기반 배치 워크플로우로 구현 가능 |
| 3 | 대표 이미지 자동 생성 후 삽입 | 가능 | 이미지 생성 API 호출 후 저장소 자산으로 관리 |
| 4 | GitHub Public에 글·이미지 저장, Pages로 배포 | 가능 | 정적 사이트와 매우 잘 맞음 |
| 5 | 특정 폴더의 docs/docx 자동 변환 게시 | 가능 | Markdown은 즉시, DOCX는 Rust 파서 또는 변환기 구현 |
| 6 | 모든 구현 Rust 기반 WASM | 가능하나 고난도 | WASI + Wasmtime + wasi:http 설계 필요 |

---

## 3. 시스템 전체 구조

```text
입력
 ├─ concepts/*.yaml 또는 concepts/*.md
 ├─ imports/docs/*.md
 └─ imports/docx/*.docx

GitHub Actions
 ├─ 문서 변경 감지
 ├─ WASM 문서 변환 모듈 실행
 ├─ WASM AI 파이프라인 실행
 │   ├─ 리서치 프롬프트 구성
 │   ├─ 모델별 초안 생성 요청
 │   ├─ 결과 비교평가
 │   ├─ 통합·보강
 │   └─ 최종 포스트 산출
 ├─ WASM 이미지 프롬프트 생성
 ├─ 이미지 생성 API 호출
 ├─ Markdown/Post JSON 생성
 ├─ 정적 사이트 빌드
 ├─ 생성물 저장소 커밋
 └─ GitHub Pages 배포

출력
 ├─ content/posts/*.md
 ├─ static/images/posts/*
 ├─ public/*
 └─ GitHub Pages 블로그
```

---

## 4. 기술 아키텍처

## 4.1 계층별 구성

### A. 콘텐츠 입력 계층
- `concepts/`
  - 사람이 작성하는 컨셉 초안
  - YAML 또는 Markdown front matter 사용
- `imports/docs/`
  - 기존 Markdown 문서
- `imports/docx/`
  - Word 문서

### B. 변환·정규화 계층
- `doc-ingest.wasm`
  - Markdown 파일 메타데이터 정리
  - DOCX 파싱
  - 제목, 본문, 표, 이미지 참조를 표준 Markdown으로 변환
  - 변환 결과를 `content/inbox/`에 저장

### C. AI 오케스트레이션 계층
- `ai-pipeline.wasm`
  - 컨셉 읽기
  - 조사 범위 도출
  - AI 모델 A/B/C에 각각 프롬프트 전달
  - 각 모델의 답변을 구조화(JSON)
  - 평가 모델 또는 규칙 기반 루브릭으로 비교
  - 최종 통합 원고 작성
  - SEO 메타데이터와 요약 생성

### D. 이미지 생성 계층
- `image-brief.wasm`
  - 최종 글에서 대표 이미지 프롬프트 생성
  - 블로그 썸네일용 프롬프트와 alt text 생성
- 외부 이미지 생성 API
  - 생성된 대표 이미지 저장
  - 경로를 포스트 front matter에 자동 삽입

### E. 사이트 생성 계층
- `sitegen.wasm`
  - Markdown을 HTML로 변환
  - 홈, 글 목록, 태그, 카테고리, 검색 인덱스, RSS, sitemap 생성
  - 결과물을 `public/`에 생성

### F. 프런트엔드 계층
- `web-ui.wasm`
  - 검색
  - 태그 필터
  - 테마 전환
  - 목차 펼침
  - 이미지 라이트박스
  - 클라이언트 측 상호작용

---

## 5. GitHub Repository 구조

```text
.
├─ .github/
│  └─ workflows/
│     ├─ generate-post.yml
│     ├─ import-documents.yml
│     └─ deploy-pages.yml
│
├─ concepts/
│  ├─ 2026-05-ai-blog-platform.yaml
│  └─ ...
│
├─ imports/
│  ├─ docs/
│  └─ docx/
│
├─ content/
│  ├─ inbox/
│  ├─ drafts/
│  └─ posts/
│
├─ static/
│  ├─ images/
│  │  └─ posts/
│  └─ css/
│
├─ public/
│
├─ crates/
│  ├─ core-types/
│  ├─ doc-ingest/
│  ├─ ai-pipeline/
│  ├─ image-brief/
│  ├─ sitegen/
│  ├─ web-ui/
│  └─ common-utils/
│
├─ wit/
│  ├─ ai-provider.wit
│  ├─ http-client.wit
│  └─ storage.wit
│
├─ config/
│  ├─ site.toml
│  ├─ ai-providers.toml
│  └─ evaluation-rubric.toml
│
├─ scripts/
│  └─ local-dev.md
│
├─ Cargo.toml
└─ README.md
```

---

## 6. 블로그 주요 기능 설계

### 6.1 필수 페이지
- 홈
- 전체 글 목록
- 카테고리별 목록
- 태그별 목록
- 단일 포스트 상세
- 소개/About
- 검색 페이지
- RSS
- sitemap.xml
- 404

### 6.2 포스트 메타데이터
```yaml
---
title: "AI 기반 블로그 자동화 구축기"
slug: "ai-blog-automation"
date: "2026-05-17"
updated: "2026-05-17"
summary: "GitHub Actions와 Rust/WASM으로 블로그 자동화를 구현하는 방법"
tags:
  - github-actions
  - rust
  - wasm
  - ai
category: "Development"
hero_image: "/images/posts/ai-blog-automation.webp"
hero_alt: "자동화된 글쓰기와 정적 사이트 생성 과정을 표현한 일러스트"
status: "published"
source:
  type: "concept"
  path: "concepts/2026-05-ai-blog-platform.yaml"
---
```

---

## 7. AI 글쓰기 파이프라인

## 7.1 입력 컨셉 예시
```yaml
title: "개발자 개인 블로그를 GitHub Actions로 대체하기"
audience: "개발자, 1인 창업자, 기술 블로거"
goal: "기술 검토와 구현 방향 제시"
tone: "전문적이되 이해하기 쉬운 설명"
length: "3500-5000자"
keywords:
  - github actions
  - rust wasm
  - ai writer
  - github pages
references:
  policy: "공식 문서 우선"
```

## 7.2 다단계 생성 흐름
1. 컨셉 분석
2. 조사 질문 자동 생성
3. 지정 AI 모델별 조사 메모 생성
4. 모델별 초안 작성
5. 초안 정규화
6. 비교 평가
7. 장점 통합
8. 사실 누락 확인
9. 최종 원고 생성
10. 메타데이터·요약·SEO 제목 생성
11. 대표 이미지 프롬프트 생성
12. 이미지 생성 후 글에 자동 연결

---

## 8. 다중 AI 비교평가 설계

## 8.1 평가 항목
- 사실성
- 구조 명확성
- 독자 적합성
- 전문성
- 표현력
- 중복 제거
- 실무 활용도
- 제목 매력도
- SEO 요소
- 출처와 주장 정합성

## 8.2 평가 출력 스키마
```json
{
  "candidate_id": "model-a",
  "scores": {
    "accuracy": 8,
    "structure": 9,
    "readability": 8,
    "practicality": 9,
    "seo": 7
  },
  "strengths": [
    "구성이 명확함",
    "예시가 실무적임"
  ],
  "weaknesses": [
    "도입부가 평범함",
    "이미지 활용 제안이 약함"
  ],
  "recommended_revisions": [
    "도입부 문제 제기를 강화",
    "배포 아키텍처 설명 추가"
  ]
}
```

---

## 9. 이미지 자동 생성 설계

### 9.1 생성 타이밍
- 최종 본문이 확정된 직후

### 9.2 이미지 생성 입력
- 글 제목
- 핵심 주제
- 카테고리
- 스타일 규칙
- 브랜드 톤
- 시각적 금지 요소

### 9.3 생성 이미지 결과물
- `static/images/posts/{slug}.webp`
- `static/images/posts/{slug}-social.webp`
- alt text 자동 생성
- front matter 자동 업데이트

### 9.4 권장 기본 정책
- 썸네일 16:9
- 본문 대표 이미지 3:2 또는 16:9
- 파일명은 slug 기반
- 과도한 텍스트 삽입 금지
- 라이선스와 정책상 문제가 없는 API 사용

---

## 10. 문서 자동 변환 설계

## 10.1 Markdown 파일
- `imports/docs/*.md`
- front matter 유무 검사
- 없으면 기본 메타데이터 자동 생성
- `content/posts/` 또는 `content/drafts/`로 이동

## 10.2 DOCX 파일
- `imports/docx/*.docx`
- DOCX를 ZIP + XML 구조로 파싱
- 제목/단락/리스트/표/이미지 관계를 읽음
- Markdown으로 변환
- 필요 시 추출 이미지는 `static/images/imported/`에 저장
- 품질 검증 후 `content/drafts/` 생성
- 자동 게시 여부는 config로 제어

## 10.3 DOCX 처리 정책
```toml
[docx]
publish_mode = "draft"    # draft | publish
extract_images = true
convert_tables = true
preserve_headings = true
```

---

## 11. GitHub Actions 워크플로우 설계

## 11.1 generate-post.yml
트리거:
- `concepts/**` 변경
- 수동 실행

역할:
- WASM AI 파이프라인 실행
- 최종 포스트 생성
- 대표 이미지 생성
- 결과 커밋

## 11.2 import-documents.yml
트리거:
- `imports/docs/**`
- `imports/docx/**`

역할:
- WASM 문서 변환기 실행
- Markdown 포스트 생성
- 결과 커밋

## 11.3 deploy-pages.yml
트리거:
- `content/posts/**`
- `static/images/**`
- 수동 실행

역할:
- WASM 정적 사이트 생성기 실행
- `public/` 생성
- GitHub Pages에 배포

---

## 12. 보안 및 운영 정책

### 12.1 Secrets
- AI API KEY
- 이미지 생성 API KEY
- 선택적으로 검색 API KEY
- GitHub Actions Secret으로 관리

### 12.2 권한 최소화
- Pages 배포에는 필요한 권한만 부여
- 저장소 커밋이 필요한 워크플로우만 `contents: write`
- 나머지는 `contents: read`

### 12.3 공개 저장소 주의
- 입력 컨셉, 초안, 이미지 모두 공개될 수 있음
- 비공개 자료를 넣지 않도록 가이드 필요
- 민감한 조사 데이터는 저장 금지

---

## 13. 핵심 기술 선택

## 13.1 정적 사이트 생성
### 권장
- 자체 `sitegen.wasm`
  - 요구사항 6을 가장 엄격히 충족
  - 구조와 산출물을 완전히 통제 가능

### 보조 검토
- Dioxus SSG
  - Rust 기반 정적 사이트 생성 경험이 좋음
  - 다만 엄격한 “전부 WASM 실행” 조건과는 완벽히 일치하지 않을 수 있음

## 13.2 브라우저 UI
- Rust/WASM 기반으로 구현
- 검색, 필터, 인터랙션 담당

## 13.3 배치 실행
- Wasmtime 사용
- 배치 로직은 WASI 모듈로 실행

## 13.4 HTTP/API 호출
선택지:
1. `wasi:http` 기반 컴포넌트
2. 별도의 매우 얇은 host wrapper + WASM 로직
3. 엄격한 제약을 완화하여 네이티브 Rust CLI 사용

본 기획은 1안을 기본 가정으로 하지만, 구현 리스크를 줄이려면 2안을 현실적인 절충안으로 고려한다.

---

## 14. 구현 단계

## Phase 1. 최소 블로그 MVP
- 저장소 구조 생성
- Markdown 포스트 렌더링
- 홈/목록/상세 페이지
- Pages 배포
- RSS/sitemap

## Phase 2. 문서 자동 게시
- `imports/docs/` 자동 게시
- DOCX 변환기 1차 구현
- draft/publish 정책 도입

## Phase 3. AI 글쓰기 파이프라인
- 컨셉 파일 스키마 정의
- 모델 1개 연결
- 단일 초안 생성
- 최종 Markdown 생성

## Phase 4. 다중 AI 비교평가
- 모델 A/B/C 병렬 호출
- 평가 루브릭 적용
- 통합/보강 원고 생성
- 실패 재시도 정책

## Phase 5. 이미지 자동 생성
- 대표 이미지 프롬프트 생성
- 이미지 생성 API 연동
- 자동 저장 및 포스트 연결

## Phase 6. 운영 고도화
- 캐싱
- 실패 로그
- 품질 점수
- 초안 검수 옵션
- 수동 승인 워크플로우

---

## 15. 품질 기준

### 기능 품질
- 새 컨셉 파일을 추가하면 1회 실행으로 포스트와 이미지가 생성되어야 함
- 새 DOCX를 넣으면 최소 draft가 자동 생성되어야 함
- 배포 결과는 GitHub Pages에서 정상 접근 가능해야 함

### 콘텐츠 품질
- 문장 반복 최소화
- 출처 없는 단정적 주장 최소화
- SEO 메타 자동 생성
- 이미지 alt text 누락 금지

### 코드 품질
- 모든 커스텀 로직은 Rust
- 목표별 WASM 타깃 명확화
- 테스트 가능한 순수 함수 중심 설계
- 스키마 검증
- 실패 시 명시적 에러 메시지

---

## 16. 리스크와 대응

| 리스크 | 설명 | 대응 |
|---|---|---|
| WASM 환경에서 HTTP 통신 복잡 | AI API 호출이 어려워질 수 있음 | wasi:http 또는 host wrapper 설계 |
| DOCX 변환 품질 편차 | 복잡한 표/스타일/각주 처리 난이도 | 초기는 핵심 기능 우선, 차후 확장 |
| AI 출력 일관성 | 초안 품질 편차 큼 | JSON 스키마, 평가 루브릭, 재생성 정책 |
| 공개 저장소 보안 | 민감정보 노출 위험 | 입력 정책, Secret 사용, 검수 단계 |
| Actions 비용/시간 | AI 생성과 이미지 생성으로 지연 가능 | 캐싱, 변경분만 처리 |

---

## 17. 권장 최종 아키텍처

### 엄격 준수형
```text
GitHub Actions
  ├─ Wasmtime 실행
  ├─ doc-ingest.wasm
  ├─ ai-pipeline.component.wasm
  ├─ image-brief.wasm
  ├─ sitegen.wasm
  └─ Pages 배포
```

### 현실 절충형
```text
GitHub Actions
  ├─ Rust native CLI 일부 사용
  ├─ 사이트/UI는 Rust/WASM
  ├─ 핵심 파싱·평가 로직은 공용 Rust crate
  └─ Pages 배포
```

### 본 프로젝트의 기본 추천
- **1차 구현은 현실 절충형**
- **2차 고도화에서 엄격 준수형으로 수렴**
- 다만 사용자가 “반드시 전부 WASM 실행”을 절대 조건으로 유지한다면, 초기부터 엄격 준수형으로 설계한다.

---

## 18. 완료 정의(Definition of Done)

다음 조건을 만족하면 1차 성공으로 본다.

1. GitHub Public 저장소 존재
2. GitHub Pages 자동 배포 성공
3. `content/posts/`의 Markdown이 실제 블로그로 게시
4. `imports/docs/` 문서 자동 반영
5. `imports/docx/` 문서가 draft Markdown으로 변환
6. `concepts/` 파일 하나로 AI 블로그 포스트 생성
7. 대표 이미지 자동 생성 및 글 삽입
8. 주요 커스텀 로직이 Rust 기반이며 WASM 실행 경로가 확보됨

---

## 19. 추천 개발 순서

1. 저장소 구조와 사이트 생성기
2. Pages 배포
3. Markdown 문서 자동 게시
4. DOCX 변환
5. 단일 AI 글쓰기
6. 다중 AI 평가 통합
7. 이미지 생성
8. 운영 안정화
