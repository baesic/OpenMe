---
title: OpenMe 시작하기
slug: welcome-openme
date: 2026-05-17
updated: 2026-05-17
summary: Rust/WASM과 GitHub Actions로 자동 블로그 파이프라인을 구성하는 OpenMe의 시작 글입니다.
tags:
- openme
- rust
- wasm
category: Project
hero_image: /images/posts/welcome-openme.webp
hero_image_prompt: 16:9 editorial illustration of a Rust and WebAssembly powered publishing workflow on GitHub Pages, clean technical style, no text
hero_alt: Rust와 WebAssembly 기반 블로그 자동화 흐름을 표현한 대표 이미지
status: published
source:
  type: seed
  path: content/posts/welcome-openme.md
---

# OpenMe 시작하기

OpenMe는 GitHub Public Repository와 GitHub Actions, GitHub Pages를 이용해 블로그 운영을 자동화하는 Rust/WASM 기반 프로젝트입니다.

## 무엇을 자동화하나

컨셉 문서가 들어오면 조사 질문을 만들고, 여러 작성자 후보의 초안을 정규화한 뒤, 평가 루브릭으로 비교하여 최종 글을 생성합니다. 문서 폴더의 Markdown과 DOCX는 별도 ingest 단계에서 초안 또는 게시글로 변환됩니다.

## 왜 Rust/WASM인가

사이트 생성, 문서 변환, AI 파이프라인, 이미지 메타데이터 보정 같은 사용자 정의 로직을 Rust crate로 분리하고 GitHub Actions에서는 Wasmtime으로 WASI 모듈을 실행합니다. 브라우저 검색과 필터링 로직도 Rust에서 WebAssembly로 빌드한 모듈을 사용합니다.

## 운영 원칙

API 키는 코드나 공개 문서에 저장하지 않습니다. 외부 AI와 이미지 생성 API는 GitHub Actions Secret과 host adapter 계층으로 연결하고, 저장소에는 최종 게시물과 공개 가능한 이미지 자산만 남깁니다.
