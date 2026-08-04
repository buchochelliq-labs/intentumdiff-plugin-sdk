//! IntentumDiff Plugin SDK
//!
//! Provides the types and helpers needed to implement parser or renderer
//! plugins for IntentumDiff.
//!
//! # Getting started
//!
//! Add this crate as a git dependency in your plugin's `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! intentumdiff-plugin-sdk = { git = "https://github.com/buchochelliq-labs/intentumdiff-plugin-sdk", tag = "v0.1.0" }
//! ```
//!
//! Then implement the `parser` or `renderer` WIT interface.

pub mod cst;
pub mod hash;
pub mod metadata;
pub mod tree;
#[cfg(feature = "ts-convert")]
pub mod ts_convert;

pub use cst::{CstNode, CstNodeKind};
pub use hash::{structural_hash, structural_hash_with_memo, COMMON_TRIVIA};
pub use metadata::{parse_plugin_metadata, LanguageMetadata, PluginMetadata};
pub use tree::{SemanticNode, SemanticNodeBuilder, SemanticTree};

/// Re-export serde_json for convenience so plugins don't need to depend on it
/// separately.
pub use serde_json;

/// Test helpers and the [`plugin_compliance_tests!`] macro for plugin authors.
/// Enabled via `features = ["testing"]` in `[dev-dependencies]`.
#[cfg(feature = "testing")]
pub mod testing;
