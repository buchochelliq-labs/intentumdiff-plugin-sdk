# Changelog

## Unreleased — v0.0.3

- **Compliance no longer certifies a parser that never returns a tree** (#24).
  `plugin_compliance_tests!` requires a valid `SemanticTree` (not an error envelope, `null`,
  a scalar or `{}`) for empty/whitespace input, adds hostile-input and determinism tests and
  optional `samples:` / `exported_grammar_id:`+`exported_language_ids:` sections; the checks
  are public in `testing`. `parser-ci.yml` verifies the built `.wasm` exports
  `intentdiff:plugin/parser@1.0.0` and fails when a required `compliance_*` test is absent
  or ignored. The template emits 0-based positions.

## v0.1.0 — 2026-07-26

Initial import from the IntentumDiff monorepo (files-only; the monorepo remains the archive of
record): the `intentdiff:plugin` WIT contract, the SDK library (+ `sql-parser-lib`), the
cargo-generate plugin template (manifest as `Cargo.toml.liquid`), the mastered plugin skills,
and the reusable parser CI workflow.
