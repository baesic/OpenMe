---
title: OpenMe Markdown 가져오기 샘플
slug: openme-markdown-가져오기-샘플
date: 2026-05-17
updated: 2026-05-17
summary: 이 파일은 doc-ingest가 Markdown 문서를 draft 또는 post로 변환하는 흐름을 검증하기 위한 샘플입니다. - front matter가 없으면 제목, slug, 날짜, 요약을 자동 보정합니다. - 공개 저장소에 들어갈 수 없는 민감한 자료는 입력하지 않습니다. - co...
category: Imported
status: draft
source:
  type: markdown-import
  path: imports/docs/sample-import.md
---

# OpenMe Markdown 가져오기 샘플

이 파일은 `doc-ingest`가 Markdown 문서를 draft 또는 post로 변환하는 흐름을 검증하기 위한 샘플입니다.

## 핵심 내용

- front matter가 없으면 제목, slug, 날짜, 요약을 자동 보정합니다.
- 공개 저장소에 들어갈 수 없는 민감한 자료는 입력하지 않습니다.
- `config/site.toml`의 게시 정책에 따라 `content/drafts/` 또는 `content/posts/`로 출력합니다.
