# Changelog

## v0.0.2-beta.1 — 2026-08-08

Versioning unified across the IntentumDiff components: every component ships
`0.0.2-beta.1` for this release, spelled per its ecosystem (`0.0.2b1` for the Python
wheel). Previously released as v0.2.0 in this changelog; the tag cut is
`v0.0.2-beta.1`.

## v0.2.0 — 2026-08-04

**Rebrand: IntentDiff is now IntentumDiff.** The crate is renamed
`intentdiff-plugin-sdk` -> `intentumdiff-plugin-sdk`.

This release exists because a rename cannot be applied retroactively to a tag. Consumers
pin the SDK by git tag, and `v0.1.0` will forever contain a package named
`intentdiff-plugin-sdk`. After the rebrand every component's manifest referenced
`intentumdiff-plugin-sdk` while still pinning `v0.1.0`, so the build failed with:

    error: no matching package named `intentumdiff-plugin-sdk` found

The dependency key was renamed; the pinned tag was not. Cutting a tag whose package name
matches is the only fix — the tag is immutable, which is exactly why it is trustworthy.

### Not changed, deliberately

The **WIT package namespace stays `intentdiff:plugin@1.0.0`**. It is a wire contract, not a
brand: all 69 certified components are compiled against it and pinned by SHA-256 in the
registry. Renaming it made the host expect a namespace no existing component exports, which
produced 637 test failures whose symptom named the wrong layer entirely. An ABI identifier
has to outlive the brand.

### Upgrading

    intentumdiff-plugin-sdk = { git = "...", tag = "v0.0.2-beta.1" }

No source changes are required: only the package name moved.

## v0.1.0 — 2026-07-26

Initial import from the IntentumDiff monorepo (files-only; the monorepo remains the archive of
record): the `intentdiff:plugin` WIT contract, the SDK library (+ `sql-parser-lib`), the
cargo-generate plugin template (manifest as `Cargo.toml.liquid`), the mastered plugin skills,
and the reusable parser CI workflow.
