# intentumdiff-plugin-sdk architecture

The root of the plugin dependency DAG. Three responsibilities:

1. **The contract.** `wit/plugin.wit` (`intentumdiff:plugin`) is mastered here; every plugin
   implements it and every host loads through it. Consumers commit their crate-local copy;
   contract changes are versioned here (the host gates on `abi_target` compatibility).
2. **The SDK library** (`crates/sdk`): `SemanticNode`/`SemanticNodeBuilder`/`SemanticTree`,
   structural hashing, CST types, plugin-metadata parsing, the shared tree-sitter→CST
   conversion (`ts-convert`), and the compliance-test macro (`testing`). `crates/sql-parser-lib`
   adds shared SQL utilities for SQL-family parsers.
3. **The fan-out machinery.** This repo masters the plugin skills (`.claude/skills/`) and the
   reusable parser CI (`.github/workflows/parser-ci.yml`) — parser repos carry stamped copies /
   thin callers, so a fix lands once here, not 69 times.

`crates/patches/` vendors the `[patch.crates-io]` crates the SDK's own graph needs
(build-script stabilization); grammar patches live with each parser repo.

Template note: the cargo-generate template's manifest is `Cargo.toml.liquid` so cargo's
git-dependency package discovery never parses the `{{project-name}}` placeholder.
