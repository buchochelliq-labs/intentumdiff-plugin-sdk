//! Test utilities for IntentumDiff parser and renderer plugin authors.
//!
//! This module provides assertion helpers and the [`plugin_compliance_tests!`]
//! macro that generates a standard compliance test suite for any parser plugin.
//!
//! # Quick start
//!
//! In your plugin crate's `Cargo.toml`:
//! ```toml
//! [dev-dependencies]
//! intentumdiff-plugin-sdk = { path = "../../crates/sdk", features = ["testing"] }
//! ```
//!
//! In `src/lib.rs`, extract your logic into testable functions:
//! ```rust,ignore
//! fn detect_language_impl(filename: &str, content: &str) -> String { … }
//! fn parse_mylang(source: &str) -> String { … }
//! ```
//!
//! Then add a test module:
//! ```rust,ignore
//! #[cfg(test)]
//! mod tests {
//!     use super::*;
//!
//!     intentumdiff_plugin_sdk::plugin_compliance_tests! {
//!         process: parse_mylang,
//!         detect_fn: detect_language_impl,
//!         detect_cases: [
//!             ("file.myext", "", "mylang"),
//!             ("file.txt",   "", ""),
//!         ],
//!         grammar_id: "mylang",
//!         language_ids: ["mylang"],
//!     }
//!
//!     #[test]
//!     fn my_specific_test() { … }
//! }
//! ```

use serde_json::Value;

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// Assert that `json_str` parses as valid JSON, panicking with a descriptive
/// message that includes the first 200 characters of the output on failure.
pub fn assert_valid_json(json_str: &str, context: &str) {
    if let Err(e) = serde_json::from_str::<Value>(json_str) {
        panic!(
            "{}: expected valid JSON, got parse error: {}\n  output (first 200 chars): {:?}",
            context,
            e,
            &json_str[..json_str.len().min(200)]
        );
    }
}

/// Assert that the JSON output does **not** contain a top-level `"error"` key.
pub fn assert_no_error(json_str: &str, context: &str) {
    let v: Value = serde_json::from_str(json_str)
        .unwrap_or_else(|e| panic!("{}: invalid JSON: {}", context, e));
    if let Some(err) = v.get("error") {
        panic!("{}: unexpected error field: {}", context, err);
    }
}

/// Assert that the root node of the output tree has `node_type == expected`.
pub fn assert_root_node_type(json_str: &str, expected: &str, context: &str) {
    let v: Value = serde_json::from_str(json_str)
        .unwrap_or_else(|e| panic!("{}: invalid JSON: {}", context, e));
    let actual = v
        .get("node_type")
        .and_then(|t| t.as_str())
        .unwrap_or("(missing)");
    assert_eq!(
        actual, expected,
        "{}: root node_type expected {:?}, got {:?}",
        context, expected, actual
    );
}

/// Assert that the tree contains **at least one** node anywhere whose
/// `node_type` equals `expected`.
pub fn assert_contains_node_type(json_str: &str, expected: &str, context: &str) {
    let v: Value = serde_json::from_str(json_str)
        .unwrap_or_else(|e| panic!("{}: invalid JSON: {}", context, e));
    assert!(
        find_node_type(&v, expected),
        "{}: expected at least one node of type {:?} but found none\n  tree: {}",
        context,
        expected,
        json_str
    );
}

/// Assert that the root node has exactly `expected` direct children.
pub fn assert_child_count(json_str: &str, expected: usize, context: &str) {
    let v: Value = serde_json::from_str(json_str)
        .unwrap_or_else(|e| panic!("{}: invalid JSON: {}", context, e));
    let actual = v
        .get("children")
        .and_then(|c| c.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(
        actual, expected,
        "{}: expected {} root children, got {}",
        context, expected, actual
    );
}

/// Assert that every node of the given `node_type` anywhere in the tree has a
/// non-empty `label` field.
pub fn assert_labels_nonempty(json_str: &str, node_type: &str, context: &str) {
    let v: Value = serde_json::from_str(json_str)
        .unwrap_or_else(|e| panic!("{}: invalid JSON: {}", context, e));
    check_labels_nonempty(&v, node_type, context);
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn find_node_type(v: &Value, node_type: &str) -> bool {
    if v.get("node_type").and_then(|t| t.as_str()) == Some(node_type) {
        return true;
    }
    if let Some(children) = v.get("children").and_then(|c| c.as_array()) {
        for child in children {
            if find_node_type(child, node_type) {
                return true;
            }
        }
    }
    false
}

fn check_labels_nonempty(v: &Value, node_type: &str, context: &str) {
    if v.get("node_type").and_then(|t| t.as_str()) == Some(node_type) {
        let label = v.get("label").and_then(|l| l.as_str()).unwrap_or("");
        assert!(
            !label.is_empty(),
            "{}: node of type {:?} has an empty label",
            context,
            node_type
        );
    }
    if let Some(children) = v.get("children").and_then(|c| c.as_array()) {
        for child in children {
            check_labels_nonempty(child, node_type, context);
        }
    }
}

// ---------------------------------------------------------------------------
// Compliance test macro
// ---------------------------------------------------------------------------

/// Generate a standard set of compliance tests for a parser plugin.
///
/// Invoke this macro inside a `#[cfg(test)] mod tests { use super::*; … }` block.
///
/// ## Required parameters
///
/// * `process: <path>` — path to a `fn(source: &str) -> String` parse function.
/// * `detect_fn: <path>` — path to a `fn(filename: &str, content: &str) -> String`.
/// * `detect_cases: [(filename, content, expected_lang), …]` — detection assertions.
///   Use `""` for `expected_lang` to assert the parser does **not** claim the file.
/// * `grammar_id: <str literal>` — the grammar ID the plugin reports.
/// * `language_ids: [<str literal>, …]` — all language IDs the plugin handles.
///
/// ## Generated tests
///
/// | Test name | What it checks |
/// |---|---|
/// | `compliance_grammar_id_nonempty` | `grammar_id` is not the empty string |
/// | `compliance_language_ids_not_empty` | `language_ids` has at least one entry |
/// | `compliance_language_ids_contain_grammar_id` | `grammar_id` appears in `language_ids` |
/// | `compliance_process_empty_valid_json` | `process("")` → valid JSON |
/// | `compliance_process_whitespace_valid_json` | `process(" \n\t ")` → valid JSON |
/// | `compliance_detect_language_cases` | all supplied `detect_cases` pass |
#[macro_export]
macro_rules! plugin_compliance_tests {
    (
        process: $process_fn:path,
        detect_fn: $detect_fn:path,
        detect_cases: [$( ($dc_file:expr, $dc_content:expr, $dc_expected:expr) ),* $(,)?],
        grammar_id: $grammar_id:expr,
        language_ids: [$($lang_id:expr),* $(,)?],
    ) => {
        #[test]
        fn compliance_grammar_id_nonempty() {
            assert!(!$grammar_id.is_empty(), "grammar_id must not be empty");
        }

        #[test]
        fn compliance_language_ids_not_empty() {
            let ids: &[&str] = &[$($lang_id),*];
            assert!(!ids.is_empty(), "language_ids must not be empty");
        }

        #[test]
        fn compliance_language_ids_contain_grammar_id() {
            let ids: &[&str] = &[$($lang_id),*];
            assert!(
                ids.contains(&$grammar_id),
                "language_ids {:?} must contain grammar_id {:?}",
                ids,
                $grammar_id
            );
        }

        #[test]
        fn compliance_process_empty_valid_json() {
            let out = $process_fn("");
            intentumdiff_plugin_sdk::testing::assert_valid_json(&out, "process(empty)");
        }

        #[test]
        fn compliance_process_whitespace_valid_json() {
            let out = $process_fn("   \n\t  ");
            intentumdiff_plugin_sdk::testing::assert_valid_json(&out, "process(whitespace)");
        }

        #[test]
        fn compliance_detect_language_cases() {
            $(
                {
                    let got = $detect_fn($dc_file, $dc_content);
                    assert_eq!(
                        got.as_str(),
                        $dc_expected,
                        "detect_language({:?}, {:?}) → {:?}, expected {:?}",
                        $dc_file,
                        $dc_content,
                        got,
                        $dc_expected
                    );
                }
            )*
        }
    };
}
