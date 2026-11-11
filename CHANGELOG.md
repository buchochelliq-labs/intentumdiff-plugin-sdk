# Changelog

## Unreleased — v0.0.3

- **Compliance no longer certifies a parser that never returns a tree** (#24).
  `plugin_compliance_tests!` requires a valid `SemanticTree` (not an error envelope, `null`,
  a scalar or `{}`) for empty/whitespace input, adds hostile-input and determinism tests and
  optional `samples:` / `exported_grammar_id:`+`exported_language_ids:` sections; the checks
  are public in `testing`. `parser-ci.yml` verifies the built `.wasm` exports
  `intentdiff:plugin/parser@1.0.0` and fails when a required `compliance_*` test is absent
  or ignored. The template emits 0-based positions.
- **Opt-in identity for CST hashes** (#25), additive — defaults are byte-identical:
  `ts_convert::node_to_cst_with` + `CstOptions`/`AnonymousTokens` keep anonymous
  operator/keyword tokens; `hash::HashOptions` (`literal_text`, `is_trivia`;
  `HashOptions::IDENTITY`) hashes internal literal payload and excludes comments, via
  `structural_hash_with_options` and `convert_semantic{,_classed,_strict}_with`;
  `literal_label_full` drops the 120-char display cap. See PLUGIN_GUIDE "Identity".
- **Documented setup resolves** (#27): the SDK version is `0.0.3-beta.1` and the template,
  README, BUILDING and crate docs pin tag `v0.0.3-beta.1` (`v0.1.0` still names the
  pre-rebrand `intentdiff-plugin-sdk` package). The template is a real parser component
  (WIT copy, `Guest` exports, metadata, compliance tests) and is compiled and tested in CI
  via `tests/authoring-standard`.

## v0.1.0 — 2026-07-26

Initial import from the IntentumDiff monorepo (files-only; the monorepo remains the archive of
record): the `intentdiff:plugin` WIT contract, the SDK library (+ `sql-parser-lib`), the
cargo-generate plugin template (manifest as `Cargo.toml.liquid`), the mastered plugin skills,
and the reusable parser CI workflow.
