# Local Development

```sh
rustup target add wasm32-wasip1 wasm32-unknown-unknown
cargo test --workspace
cargo build --release --target wasm32-wasip1 -p sitegen -p doc-ingest -p ai-pipeline -p image-brief
cargo build --release --target wasm32-unknown-unknown -p web-ui
wasmtime --dir . target/wasm32-wasip1/release/sitegen.wasm -- --config config/site.toml
mkdir -p public/wasm
cp target/wasm32-unknown-unknown/release/openme_web_ui.wasm public/wasm/openme_web_ui.wasm
```

정적 결과물은 `public/index.html`에서 확인할 수 있습니다.
