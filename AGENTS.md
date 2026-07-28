# Agent instructions — intentdiff-plugin-sdk

The plugin **contract + SDK**: root of the plugin dependency DAG.

## Hard invariants
- `wit/plugin.wit` changes are ecosystem events (host gates on `abi_target`); keep additive.
- This repo MASTERS the plugin skills (`.claude/skills/`) and the reusable parser CI
  (`parser-ci.yml`) — parser repos carry stamped copies; fixes land here, never there.
- The template manifest stays `Cargo.toml.liquid` (cargo git-dep discovery must not parse it).

## Build + test (Rust 1.93.0)
```bash
cargo test --workspace --all-features
cargo check --workspace --features wasm --target wasm32-wasip2
```

## Map
`docs/ARCHITECTURE.md` · `docs/PLUGIN_GUIDE.md` (authoring) · `docs/CST_SCHEMA.md`
(interpret-cst compat) · `docs/BUILDING.md`.
