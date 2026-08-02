# intentdiff-plugin-sdk

[![CI](https://github.com/buchochelliq-labs/intentdiff-plugin-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/buchochelliq-labs/intentdiff-plugin-sdk/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust 1.93](https://img.shields.io/badge/rust-1.93-orange.svg)](https://www.rust-lang.org/)

The **IntentDiff plugin contract and SDK** — the root of the plugin dependency graph.
Everything needed to write an IntentDiff parser or renderer plugin in Rust lives here:

- **`wit/plugin.wit`** — the canonical WIT contract (`intentdiff:plugin`). Every plugin
  implements this interface; every host loads plugins through it. This repo masters the
  contract — plugin repos consume it from here.
- **`crates/sdk`** (`intentdiff-plugin-sdk`) — the SDK library: `SemanticNode` /
  `SemanticNodeBuilder` / `SemanticTree` tree types, structural hashing, CST types, plugin
  metadata parsing, the shared tree-sitter → CST conversion (`ts-convert` feature), and the
  `plugin_compliance_tests!` macro (`testing` feature).
- **`crates/sql-parser-lib`** — shared SQL parsing utilities linked into SQL-family parser
  plugins.
- **`templates/plugin-template/`** — a `cargo-generate` template for a new parser plugin.
- **`crates/patches/`** — vendored dependency patches (build-script stabilization) that the
  SDK dependency graph needs; wired via `[patch.crates-io]` in the workspace manifest.
- **`.claude/skills/`** — the **master** copies of the plugin development skills
  (`intentdiff-plugin-repo`, `intentdiff-parsers`). Parser repos receive stamped copies;
  edits belong here and are fanned out — never edit a parser repo's copy.

## Writing a plugin

```toml
[dependencies]
intentdiff-plugin-sdk = { git = "https://github.com/buchochelliq-labs/intentdiff-plugin-sdk", tag = "v0.1.0" }
```

Implement the `parser` (or `renderer`) interface from `wit/plugin.wit`, emit a deterministic
`SemanticNode` tree, and build for the Component Model target:

```bash
cargo build --release --target wasm32-wasip2
```

Start from `templates/plugin-template/` with `cargo generate`, and read
`.claude/skills/intentdiff-plugin-repo` for the plugin-repo rules (determinism, spans,
structural hashes, compliance tests).

## Building this repo

```bash
cargo test --workspace                      # host-side unit tests
cargo check --workspace --features wasm --target wasm32-wasip2   # plugin-target check
```

Toolchain: Rust 1.93.0 (pinned in CI); target `wasm32-wasip2` for plugin builds.

## Layout

```
wit/plugin.wit           the canonical plugin contract (this repo masters it)
crates/sdk/              the SDK library
crates/sql-parser-lib/   shared SQL utilities for SQL-family plugins
crates/patches/          vendored [patch.crates-io] crates the SDK graph needs
templates/plugin-template/  cargo-generate template for a new parser plugin
.github/workflows/ci.yml        this repo's CI
.github/workflows/parser-ci.yml reusable CI that parser repos call
```

## Provenance

Migrated files-only (no history) from the IntentDiff monorepo
(`buchochelliq-labs/intentdiff`), which remains the archive of record.

License: MIT.
