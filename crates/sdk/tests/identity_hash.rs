//! Issue #25: opt-in identity hashing. Literal containers hash their retained source text,
//! trivia (comments) can be excluded from container hashes, and the default hash stays
//! byte-identical to the certified legacy algorithm.

use std::collections::HashMap;

use intentumdiff_plugin_sdk::hash::{
    is_comment_kind, structural_hash, structural_hash_with_memo, structural_hash_with_options,
    HashOptions,
};
use intentumdiff_plugin_sdk::CstNode;

fn leaf(node_type: &str, text: &str, col: u32) -> CstNode {
    CstNode {
        node_type: node_type.to_owned(),
        named: true,
        text: Some(text.to_owned()),
        start_line: 0,
        start_col: col,
        end_line: 0,
        end_col: col + text.len() as u32,
        children: Vec::new(),
    }
}

/// The issue's reproduction: a literal whose payload is only in its retained text.
fn string_literal(source: &str) -> CstNode {
    CstNode {
        node_type: "string_literal".to_owned(),
        named: true,
        text: Some(source.to_owned()),
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: source.len() as u32,
        children: vec![leaf("escape_sequence", "\\n", 4)],
    }
}

fn identity(node: &CstNode) -> String {
    structural_hash_with_options(node, &mut HashMap::new(), &HashOptions::IDENTITY)
}

#[test]
fn literal_payload_distinguishes_identity_hashes() {
    let foo = string_literal("\"foo\\n\"");
    let bar = string_literal("\"bar\\n\"");
    // The legacy hash is unchanged (certified outputs) and still collides.
    assert_eq!(structural_hash(&foo), structural_hash(&bar));
    assert_ne!(identity(&foo), identity(&bar));
    // Deterministic.
    assert_eq!(identity(&foo), identity(&string_literal("\"foo\\n\"")));
}

#[test]
fn literal_payload_beyond_display_caps_is_identity() {
    for n in [120, 4096, 8192] {
        let old = string_literal(&format!("\"{}X\\n\"", "a".repeat(n)));
        let new = string_literal(&format!("\"{}Y\\n\"", "a".repeat(n)));
        assert_ne!(identity(&old), identity(&new), "suffix after {n} chars");
    }
}

fn container(children: Vec<CstNode>) -> CstNode {
    CstNode {
        node_type: "block".to_owned(),
        named: true,
        text: None,
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 40,
        children,
    }
}

#[test]
fn trivia_predicate_excludes_comments_from_container_hashes() {
    let a = container(vec![leaf("identifier", "x", 0), leaf("comment", "# one", 2)]);
    let b = container(vec![leaf("identifier", "x", 0), leaf("comment", "# two", 2)]);
    let none = container(vec![leaf("identifier", "x", 0)]);
    assert_ne!(structural_hash(&a), structural_hash(&b), "legacy hashes comments");
    assert_eq!(identity(&a), identity(&b));
    assert_eq!(identity(&a), identity(&none));
    // Non-trivia changes still count.
    let c = container(vec![leaf("identifier", "y", 0), leaf("comment", "# one", 2)]);
    assert_ne!(identity(&a), identity(&c));
    // A comment node hashed on its own keeps its text identity.
    assert_ne!(identity(&leaf("comment", "# one", 0)), identity(&leaf("comment", "# two", 0)));

    let custom = |kind: &str| kind == "identifier";
    let opts = HashOptions { is_trivia: Some(&custom), ..HashOptions::default() };
    assert_eq!(
        structural_hash_with_options(&a, &mut HashMap::new(), &opts),
        structural_hash_with_options(&c, &mut HashMap::new(), &opts)
    );
}

#[test]
fn comment_kinds() {
    for kind in ["comment", "line_comment", "block_comment", "doc_comment", "COMMENT"] {
        assert!(is_comment_kind(kind), "{kind}");
    }
    assert!(!is_comment_kind("identifier"));
    assert!(!is_comment_kind("string"));
}

#[test]
fn default_options_are_byte_identical_to_the_legacy_hash() {
    let trees = [
        string_literal("\"foo\\n\""),
        container(vec![leaf("identifier", "x", 0), leaf("comment", "# one", 2)]),
        container(vec![string_literal("\"a\""), container(vec![leaf("n", "1", 0)])]),
        leaf("identifier", "x", 0),
    ];
    for tree in &trees {
        let legacy = structural_hash_with_memo(tree, &mut HashMap::new());
        let default =
            structural_hash_with_options(tree, &mut HashMap::new(), &HashOptions::default());
        assert_eq!(legacy, default);
        assert_eq!(legacy, structural_hash(tree));
    }
    // Frozen legacy formulas: leaf = SHA-256(type ":" text), internal = SHA-256(type "|"
    // join("|", children)) — the internal literal's own text is NOT hashed by default.
    let x = sha256_hex("identifier:x");
    assert_eq!(structural_hash(&leaf("identifier", "x", 0)), x);
    let escape = sha256_hex("escape_sequence:\\n");
    assert_eq!(
        structural_hash(&string_literal("\"foo\\n\"")),
        sha256_hex(&format!("string_literal|{escape}"))
    );
}

fn sha256_hex(payload: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(payload.as_bytes()))
}
