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
//!         // Optional (recommended): real sources that must parse into a tree.
//!         samples: [EXAMPLE_OLD, EXAMPLE_NEW],
//!         // Optional (recommended): the functions behind the WIT exports, so the
//!         // declared ids above are checked against what the component reports.
//!         exported_grammar_id: MyParser::grammar_id,
//!         exported_language_ids: MyParser::language_ids,
//!     }
//!
//!     #[test]
//!     fn my_specific_test() { … }
//! }
//! ```

use serde_json::Value;

use crate::tree::SemanticNode;

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
// Semantic-tree conformance checks (issue #24)
// ---------------------------------------------------------------------------
//
// Each check returns `Err(diagnostic)` instead of panicking so it can be tested directly;
// the `plugin_compliance_tests!` expansion turns an `Err` into a test failure. JSON
// validity alone is NOT conformance: an error envelope, `null`, a scalar or an object that
// is not a `SemanticTree` must all fail.

fn excerpt(output: &str) -> String {
    let mut end = output.len().min(200);
    while !output.is_char_boundary(end) {
        end -= 1;
    }
    format!("{:?}", &output[..end])
}

/// Validate that `output` is a full-parse `SemanticTree` (not an error envelope, not
/// `null`/scalar JSON) and that the tree's invariants hold:
///
/// * every node has a non-empty `id`, `node_type` and `structural_hash`;
/// * ids are unique across the tree;
/// * every span is ordered (`start <= end`);
/// * when `source` is given, every position lies inside it (0-based lines; a column may
///   not exceed the byte length of its line — positions past the source mean a 1-based
///   or otherwise broken position scheme).
///
/// Returns the parsed tree on success.
pub fn validate_semantic_tree(output: &str, source: Option<&str>) -> Result<SemanticNode, String> {
    let value: Value = serde_json::from_str(output)
        .map_err(|e| format!("output is not valid JSON ({e}); output: {}", excerpt(output)))?;
    let Some(object) = value.as_object() else {
        return Err(format!(
            "expected a SemanticTree JSON object, got {}",
            excerpt(output)
        ));
    };
    if let Some(error) = object.get("error") {
        return Err(format!(
            "returned an error envelope instead of a SemanticTree: {error}"
        ));
    }
    let tree: SemanticNode = serde_json::from_value(value)
        .map_err(|e| format!("output is not a SemanticTree ({e}); output: {}", excerpt(output)))?;
    let lines: Option<Vec<&str>> = source.map(|s| s.split('\n').collect());
    let mut ids = std::collections::HashSet::new();
    validate_node(&tree, lines.as_deref(), &mut ids)?;
    Ok(tree)
}

fn validate_node<'t>(
    node: &'t SemanticNode,
    lines: Option<&[&str]>,
    ids: &mut std::collections::HashSet<&'t str>,
) -> Result<(), String> {
    if node.id.is_empty() {
        return Err(format!("a {:?} node has an empty id", node.node_type));
    }
    if !ids.insert(node.id.as_str()) {
        return Err(format!("duplicate id {:?}", node.id));
    }
    if node.node_type.is_empty() {
        return Err(format!("node {:?} has an empty node_type", node.id));
    }
    if node.structural_hash.is_empty() {
        return Err(format!("node {:?} has an empty structural_hash", node.id));
    }
    let p = &node.position;
    if (p.start_line, p.start_col) > (p.end_line, p.end_col) {
        return Err(format!(
            "node {:?} has a reversed span {}:{}..{}:{}",
            node.id, p.start_line, p.start_col, p.end_line, p.end_col
        ));
    }
    if let Some(lines) = lines {
        for (line, col) in [(p.start_line, p.start_col), (p.end_line, p.end_col)] {
            let in_bounds = lines
                .get(line as usize)
                .is_some_and(|text| col as usize <= text.len());
            if !in_bounds {
                return Err(format!(
                    "node {:?} position {line}:{col} is outside the source \
                     ({} lines; positions are 0-based)",
                    node.id,
                    lines.len()
                ));
            }
        }
    }
    for child in &node.children {
        validate_node(child, lines, ids)?;
    }
    Ok(())
}

/// `process(source)` must return a valid [`SemanticTree`](crate::tree::SemanticTree) whose
/// positions lie inside `source`. Used for empty/whitespace input, which every language
/// must accept as an (empty) document.
pub fn check_process_tree(process: impl Fn(&str) -> String, source: &str) -> Result<(), String> {
    validate_semantic_tree(&process(source), Some(source))
        .map(|_| ())
        .map_err(|e| format!("process({}): {e}", excerpt(source)))
}

/// A real source sample must parse into a valid tree with at least one child under the
/// root (a constant empty tree is not a parse) and must be deterministic.
pub fn check_sample(process: impl Fn(&str) -> String, source: &str) -> Result<(), String> {
    let tree = validate_semantic_tree(&process(source), Some(source))
        .map_err(|e| format!("sample {}: {e}", excerpt(source)))?;
    if tree.children.is_empty() {
        return Err(format!(
            "sample {}: the tree root has no children — a non-empty sample must produce \
             semantic structure",
            excerpt(source)
        ));
    }
    check_deterministic(&process, source)
}

/// Two calls with the same input must return byte-identical output.
pub fn check_deterministic(process: impl Fn(&str) -> String, source: &str) -> Result<(), String> {
    let first = process(source);
    let second = process(source);
    if first != second {
        return Err(format!(
            "process({}) is not deterministic: two calls returned different output",
            excerpt(source)
        ));
    }
    Ok(())
}

/// Built-in malformed, Unicode and long-tail inputs. Each must produce either a valid
/// tree (positions inside the input) or an in-band `{"error": "<message>"}` envelope.
pub const HOSTILE_INPUTS: &[&str] = &[
    "\u{0}\u{1}\u{2}\u{7f}",
    "(((((",
    ")]}>",
    "\"unterminated",
    "/* unterminated",
    "\u{feff}\u{200b}é🙂文字\u{202e}",
    "\r\n\r\nx\r\n",
    "\t\t\n\n\n",
];

fn hostile_corpus() -> Vec<String> {
    let mut inputs: Vec<String> = HOSTILE_INPUTS.iter().map(|s| s.to_string()).collect();
    inputs.push("a".repeat(20_000));
    inputs.push(format!("{}{}", "(".repeat(256), ")".repeat(256)));
    inputs.push(format!("{}{}", "[{".repeat(128), "}]".repeat(128)));
    inputs.push("x\n".repeat(2_000));
    inputs
}

/// Run the hostile corpus through `process`. A tree must validate; an error envelope must
/// be an object whose `"error"` is a non-empty string. Anything else (null, scalars,
/// arrays, non-JSON, a tree with broken invariants) fails.
pub fn check_hostile_inputs(process: impl Fn(&str) -> String) -> Result<(), String> {
    for input in hostile_corpus() {
        let output = process(&input);
        let is_envelope = serde_json::from_str::<Value>(&output)
            .ok()
            .and_then(|v| v.get("error").cloned());
        match is_envelope {
            Some(Value::String(message)) if !message.is_empty() => continue,
            Some(other) => {
                return Err(format!(
                    "process({}): error envelope must carry a non-empty string message, got {other}",
                    excerpt(&input)
                ))
            }
            None => {
                validate_semantic_tree(&output, Some(&input))
                    .map_err(|e| format!("process({}): {e}", excerpt(&input)))?;
            }
        }
    }
    Ok(())
}

/// The ids the plugin's WIT exports actually report must match what the compliance
/// invocation declares, and the grammar id must be lowercase ASCII without spaces.
pub fn check_exported_metadata<G, I, S>(
    declared_grammar_id: &str,
    declared_language_ids: &[&str],
    exported_grammar_id: G,
    exported_language_ids: I,
) -> Result<(), String>
where
    G: AsRef<str>,
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let grammar = exported_grammar_id.as_ref();
    if grammar != declared_grammar_id {
        return Err(format!(
            "exported grammar_id {grammar:?} differs from declared grammar_id {declared_grammar_id:?}"
        ));
    }
    check_grammar_id_format(grammar)?;
    let mut exported: Vec<String> = exported_language_ids
        .into_iter()
        .map(|s| s.as_ref().to_owned())
        .collect();
    let mut declared: Vec<String> = declared_language_ids.iter().map(|s| s.to_string()).collect();
    exported.sort();
    declared.sort();
    if exported != declared {
        return Err(format!(
            "exported language_ids {exported:?} differ from declared language_ids {declared:?}"
        ));
    }
    if exported.windows(2).any(|w| w[0] == w[1]) {
        return Err(format!("exported language_ids {exported:?} contain duplicates"));
    }
    if !exported.iter().any(|id| id == grammar) {
        return Err(format!(
            "exported language_ids {exported:?} do not contain grammar_id {grammar:?}"
        ));
    }
    Ok(())
}

/// WIT contract: a grammar id is "lowercase, ASCII, no spaces".
pub fn check_grammar_id_format(grammar_id: &str) -> Result<(), String> {
    if grammar_id.is_empty()
        || !grammar_id
            .chars()
            .all(|c| c.is_ascii() && !c.is_ascii_uppercase() && !c.is_ascii_whitespace() && !c.is_ascii_control())
    {
        return Err(format!(
            "grammar_id {grammar_id:?} must be non-empty lowercase ASCII without spaces"
        ));
    }
    Ok(())
}

#[doc(hidden)]
pub fn __fail(result: Result<(), String>) {
    if let Err(message) = result {
        panic!("{message}");
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
/// * `grammar_id: <str expr>` — the grammar ID the plugin reports.
/// * `language_ids: [<str expr>, …]` — all language IDs the plugin handles.
///
/// ## Optional parameters (in this order, after the required ones)
///
/// * `samples: [<str expr>, …]` — real sources (e.g. the WIT `example` pair). Each must
///   parse into a valid tree with at least one child, deterministically.
/// * `exported_grammar_id: <path>, exported_language_ids: <path>` — the functions behind
///   the WIT `grammar-id` / `language-ids` exports (e.g. `MyParser::grammar_id` from the
///   `Guest` impl). Returns may be `String`/`&str` and any iterable of them. The
///   declared ids are then checked against what the component really reports.
///
/// ## Generated tests
///
/// | Test name | What it checks |
/// |---|---|
/// | `compliance_grammar_id_nonempty` | `grammar_id` is non-empty lowercase ASCII without spaces |
/// | `compliance_language_ids_not_empty` | `language_ids` has at least one entry |
/// | `compliance_language_ids_contain_grammar_id` | `grammar_id` appears in `language_ids` |
/// | `compliance_process_empty_valid_json` | `process("")` → a valid `SemanticTree` (not an error envelope, `null` or scalar) |
/// | `compliance_process_whitespace_valid_json` | `process(" \n\t ")` → a valid `SemanticTree` |
/// | `compliance_process_hostile_inputs` | malformed/Unicode/long-tail inputs → a valid tree or a `{"error": "…"}` envelope |
/// | `compliance_process_deterministic` | repeated calls are byte-identical |
/// | `compliance_detect_language_cases` | all supplied `detect_cases` pass |
/// | `compliance_samples_parse` | *(with `samples`)* each sample → a non-empty valid tree |
/// | `compliance_exported_metadata` | *(with `exported_*`)* exports match the declared ids |
///
/// Tree validity (see [`validate_semantic_tree`](crate::testing::validate_semantic_tree)):
/// non-empty unique ids, non-empty `node_type`/`structural_hash`, ordered spans, and
/// 0-based positions inside the input.
#[macro_export]
macro_rules! plugin_compliance_tests {
    (
        process: $process_fn:path,
        detect_fn: $detect_fn:path,
        detect_cases: [$( ($dc_file:expr, $dc_content:expr, $dc_expected:expr) ),* $(,)?],
        grammar_id: $grammar_id:expr,
        language_ids: [$($lang_id:expr),* $(,)?],
        $( samples: [$($sample:expr),* $(,)?], )?
        $( exported_grammar_id: $exported_grammar_id:path,
           exported_language_ids: $exported_language_ids:path, )?
    ) => {
        #[allow(dead_code)]
        fn __compliance_declared_language_ids() -> ::std::vec::Vec<&'static str> {
            ::std::vec![$($lang_id),*]
        }

        #[test]
        fn compliance_grammar_id_nonempty() {
            assert!(!$grammar_id.is_empty(), "grammar_id must not be empty");
            $crate::testing::__fail($crate::testing::check_grammar_id_format($grammar_id));
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
            $crate::testing::__fail($crate::testing::check_process_tree($process_fn, ""));
        }

        #[test]
        fn compliance_process_whitespace_valid_json() {
            $crate::testing::__fail($crate::testing::check_process_tree(
                $process_fn,
                "   \n\t  ",
            ));
        }

        #[test]
        fn compliance_process_hostile_inputs() {
            $crate::testing::__fail($crate::testing::check_hostile_inputs($process_fn));
        }

        #[test]
        fn compliance_process_deterministic() {
            $crate::testing::__fail($crate::testing::check_deterministic($process_fn, ""));
            $( $(
                $crate::testing::__fail($crate::testing::check_deterministic(
                    $process_fn,
                    $sample,
                ));
            )* )?
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

        $(
            #[test]
            fn compliance_samples_parse() {
                let samples: &[&str] = &[$($sample),*];
                assert!(!samples.is_empty(), "samples: supply at least one source sample");
                for sample in samples {
                    $crate::testing::__fail($crate::testing::check_sample($process_fn, sample));
                }
            }
        )?

        $(
            #[test]
            fn compliance_exported_metadata() {
                $crate::testing::__fail($crate::testing::check_exported_metadata(
                    $grammar_id,
                    &__compliance_declared_language_ids(),
                    $exported_grammar_id(),
                    $exported_language_ids(),
                ));
            }
        )?
    };
}
