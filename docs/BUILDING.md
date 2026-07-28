# Building intentdiff-plugin-sdk

Toolchain: **Rust 1.93.0**; target `wasm32-wasip2` for plugin-target checks.

```bash
cargo test --workspace                                            # host-side tests
cargo test --workspace --all-features                             # + testing/ts-convert
cargo check --workspace --features wasm --target wasm32-wasip2    # plugin-target check
```

Tag releases (`vX.Y.Z`) — plugins depend on the SDK by git tag:

```toml
intentdiff-plugin-sdk = { git = "https://github.com/buchochelliq-labs/intentdiff-plugin-sdk", tag = "v0.1.0" }
```
