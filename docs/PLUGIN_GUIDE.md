# Writing an IntentumDiff parser plugin

A parser plugin is a **Wasm component** (WASI p2, Component Model) implementing the `parser`
interface of `wit/plugin.wit`: it receives raw source and returns a deterministic
`SemanticNode` tree the engine diffs.

## 1. Start from the template

```bash
cargo generate --path templates/plugin-template
```

The template wires the SDK dependency, the WIT binding (`wit_bindgen::generate!` reading
`wit/plugin.wit` crate-relative), and a line-based placeholder parser to replace.

## 2. Emit a correct tree

- **IDs** are dotted child-index paths from the root (`"0"`, `"0.2"`, `"0.2.1"`).
- **Positions** are 0-based rows/columns spanning the node's source.
- **Labels** are the node's human identity (a function's name, a literal's text) — stable
  under formatting-only change.
- **Structural hashes** make subtree identity: use the SDK's `structural_hash` helpers.
- **Determinism is non-negotiable**: same input → byte-identical tree. No clocks, no
  randomness, no host state (the sandbox forbids them anyway).
- Prefer the shared tree-sitter conversion (`ts-convert` feature, `ts_convert` module) over a
  hand-rolled walker when your grammar is tree-sitter based.

## 3. Ship metadata + compliance tests

- `plugin_metadata.info` declares the grammar id + language ids the host routes to you.
- Enable `features = ["testing"]` in dev-dependencies and use `plugin_compliance_tests!` —
  it pins the contract shape (parse your own WIT example, survive hostile inputs with only a
  tree, an in-band `{error}` envelope, or a typed trap — never a host crash).

## 4. Build + verify

```bash
cargo build --release --target wasm32-wasip2
cargo test
```

CI: call the SDK's reusable workflow —
`uses: buchochelliq-labs/intentumdiff-plugin-sdk/.github/workflows/parser-ci.yml@main`.

## 5. Register

Official distribution goes through
[intentumdiff-registry](https://github.com/buchochelliq-labs/intentumdiff-registry): open a PR
adding your entry (git source pinned to a commit SHA, SHA-256 `wasm_checksums` of the built
component, `abi_target`). The registry's vetting CI validates the entry; installs verify the
checksums. See its `docs/SUBMITTING.md`.

## Hard rules

- No source text in `NodeFacts` (privacy: counts/enums/flags only).
- No panics across the boundary — return the in-band error envelope for parse failures.
- Never edit the stamped `.claude/skills/` copies in a parser repo — change the masters here.
