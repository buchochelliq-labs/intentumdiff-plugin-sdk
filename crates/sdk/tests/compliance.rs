//! Issue #24: the compliance suite must not certify a parser that never returns a tree.
//!
//! Every generated `plugin_compliance_tests!` test delegates to a `testing::check_*`
//! function, so these tests exercise exactly what the macro runs, against fixtures that
//! previously passed all six generated tests.

use intentumdiff_plugin_sdk::testing::{
    check_deterministic, check_exported_metadata, check_hostile_inputs, check_process_tree,
    check_sample, validate_semantic_tree,
};
use intentumdiff_plugin_sdk::tree::SemanticNodeBuilder;

fn always_errors(_: &str) -> String {
    r#"{"error":"parser never succeeds"}"#.to_owned()
}

fn always_null(_: &str) -> String {
    "null".to_owned()
}

fn always_scalar(_: &str) -> String {
    "42".to_owned()
}

fn always_string(_: &str) -> String {
    r#""tree""#.to_owned()
}

fn empty_object(_: &str) -> String {
    "{}".to_owned()
}

fn not_json(_: &str) -> String {
    "<tree/>".to_owned()
}

/// A minimal correct parser: one `line` node per non-empty source line, 0-based byte spans.
fn line_parser(source: &str) -> String {
    let lines: Vec<&str> = source.split('\n').collect();
    let children = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            SemanticNodeBuilder::new(
                format!("0.{i}"),
                "line",
                line.trim(),
                i as u32,
                0,
                i as u32,
                line.len() as u32,
                "",
            )
            .build()
        })
        .collect();
    let end_line = (lines.len() - 1) as u32;
    let end_col = lines.last().map(|l| l.len()).unwrap_or(0) as u32;
    let root = SemanticNodeBuilder::new("0", "source_file", "fixture", 0, 0, end_line, end_col, "")
        .children(children)
        .build();
    serde_json::to_string(&root).unwrap()
}

/// Errors on anything that is not obviously well-formed — a legitimate behavior for hostile
/// inputs, as long as empty input still yields a tree.
fn strict_parser(source: &str) -> String {
    if source.chars().any(|c| c == '(' || c == '\u{0}') {
        return r#"{"error":"unbalanced input"}"#.to_owned();
    }
    line_parser(source)
}

fn nondeterministic(source: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static CALLS: AtomicU32 = AtomicU32::new(0);
    let n = CALLS.fetch_add(1, Ordering::SeqCst);
    line_parser(source).replace("\"fixture\"", &format!("\"fixture{n}\""))
}

fn expect_err(result: Result<(), String>, needle: &str) {
    let message = result.expect_err("compliance check must fail");
    assert!(
        message.contains(needle),
        "diagnostic {message:?} should mention {needle:?}"
    );
}

#[test]
fn error_envelope_parser_fails_empty_and_whitespace_checks() {
    for input in ["", "   \n\t  "] {
        expect_err(check_process_tree(always_errors, input), "parser never succeeds");
    }
}

#[test]
fn null_scalar_and_non_tree_outputs_fail_with_diagnostics() {
    expect_err(check_process_tree(always_null, ""), "null");
    expect_err(check_process_tree(always_scalar, ""), "42");
    expect_err(check_process_tree(always_string, "   "), "JSON object");
    expect_err(check_process_tree(empty_object, ""), "missing field");
    expect_err(check_process_tree(not_json, ""), "not valid JSON");
}

#[test]
fn always_error_parser_fails_samples_and_hostile_inputs() {
    expect_err(check_sample(always_errors, "x"), "error envelope");
    expect_err(check_hostile_inputs(always_null), "null");
    expect_err(check_hostile_inputs(always_scalar), "42");
}

#[test]
fn correct_parser_passes_every_check() {
    for input in ["", "   \n\t  ", "a\nb\n"] {
        check_process_tree(line_parser, input).unwrap();
    }
    check_sample(line_parser, "alpha\nbeta").unwrap();
    check_hostile_inputs(line_parser).unwrap();
    check_hostile_inputs(strict_parser).unwrap();
    check_deterministic(line_parser, "alpha\nbeta").unwrap();
}

#[test]
fn sample_must_produce_structure() {
    // A constant empty root tree is valid for empty input but not for a real sample.
    fn constant_root(_: &str) -> String {
        line_parser("")
    }
    check_process_tree(constant_root, "").unwrap();
    expect_err(check_sample(constant_root, "alpha"), "no children");
}

#[test]
fn nondeterministic_output_fails() {
    expect_err(check_deterministic(nondeterministic, "alpha"), "deterministic");
}

#[test]
fn tree_invariants_are_enforced() {
    let ok = line_parser("a\nb");
    validate_semantic_tree(&ok, Some("a\nb")).unwrap();

    let mut tree: serde_json::Value = serde_json::from_str(&ok).unwrap();
    tree["children"][1]["id"] = tree["children"][0]["id"].clone();
    expect_err(
        validate_semantic_tree(&tree.to_string(), Some("a\nb")).map(|_| ()),
        "duplicate id",
    );

    let mut tree: serde_json::Value = serde_json::from_str(&ok).unwrap();
    tree["children"][0]["structural_hash"] = "".into();
    expect_err(
        validate_semantic_tree(&tree.to_string(), None).map(|_| ()),
        "structural_hash",
    );

    let mut tree: serde_json::Value = serde_json::from_str(&ok).unwrap();
    tree["children"][0]["position"]["start_line"] = 1.into();
    tree["children"][0]["position"]["end_line"] = 0.into();
    expect_err(
        validate_semantic_tree(&tree.to_string(), None).map(|_| ()),
        "reversed",
    );

    // 1-based line numbers run past the source (the old template's bug).
    let mut tree: serde_json::Value = serde_json::from_str(&ok).unwrap();
    tree["children"][1]["position"]["start_line"] = 2.into();
    tree["children"][1]["position"]["end_line"] = 2.into();
    expect_err(
        validate_semantic_tree(&tree.to_string(), Some("a\nb")).map(|_| ()),
        "outside the source",
    );

    let mut tree: serde_json::Value = serde_json::from_str(&ok).unwrap();
    tree["node_type"] = "".into();
    expect_err(
        validate_semantic_tree(&tree.to_string(), None).map(|_| ()),
        "node_type",
    );
}

#[test]
fn exported_metadata_must_match_declared_metadata() {
    fn grammar() -> String {
        "audit".to_owned()
    }
    fn ids() -> Vec<String> {
        vec!["audit".to_owned()]
    }
    fn other_ids() -> Vec<&'static str> {
        vec!["other"]
    }
    check_exported_metadata("audit", &["audit"], grammar(), ids()).unwrap();
    expect_err(
        check_exported_metadata("other", &["other"], grammar(), ids()),
        "grammar_id",
    );
    expect_err(
        check_exported_metadata("audit", &["audit"], grammar(), other_ids()),
        "language_ids",
    );
}

mod macro_expands_with_all_sections {
    use super::line_parser;

    fn detect(filename: &str, _content: &str) -> String {
        if filename.ends_with(".audit") {
            "audit".to_owned()
        } else {
            String::new()
        }
    }
    fn grammar() -> &'static str {
        "audit"
    }
    fn ids() -> Vec<String> {
        vec!["audit".to_owned()]
    }

    intentumdiff_plugin_sdk::plugin_compliance_tests! {
        process: line_parser,
        detect_fn: detect,
        detect_cases: [("file.audit", "", "audit"), ("file.txt", "", "")],
        grammar_id: "audit",
        language_ids: ["audit"],
        samples: ["alpha\nbeta\n", "gamma"],
        exported_grammar_id: grammar,
        exported_language_ids: ids,
    }
}
