# Contributing to intentumdiff-plugin-sdk

- **Contract changes are ecosystem events.** A `wit/plugin.wit` change bumps the contract
  version; the host refuses incompatible `abi_target`s. Keep changes additive where possible
  and document migration in the changelog.
- **Skills + parser CI are mastered here.** Edit `.claude/skills/intentumdiff-plugin-repo` /
  `intentumdiff-parsers` and `.github/workflows/parser-ci.yml` in this repo only — parser repos
  receive stamped copies/thin callers; never edit theirs.
- Build + test per [docs/BUILDING.md](docs/BUILDING.md); both cargo modes green, no new
  warnings.
- Plugin authoring guidance lives in [docs/PLUGIN_GUIDE.md](docs/PLUGIN_GUIDE.md).
