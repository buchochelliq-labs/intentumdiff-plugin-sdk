//! Structural hashing — identical algorithm to the Python host so hashes are
//! consistent across the plugin boundary.
//!
//! Algorithm:
//!   leaf:     SHA-256( type + ":" + text )
//!   internal: SHA-256( type + "|" + join("|", children_hashes) )

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use crate::cst::CstNode;

/// Node type names that are commonly considered trivia across most languages.
/// Plugins may call `strip_trivia` with a subset or superset of these.
pub const COMMON_TRIVIA: &[&str] = &[
    "comment",
    "line_comment",
    "block_comment",
    "whitespace",
    "newline",
    "empty_statement",
];

/// Compute the structural hash of a `CstNode` tree.
///
/// Creates a fresh memo per call; each subtree is hashed only once.
/// For sharing a memo across multiple `convert()` calls, use
/// [`structural_hash_with_memo`] instead.
pub fn structural_hash(node: &CstNode) -> String {
    let mut memo = HashMap::new();
    structural_hash_with_memo(node, &mut memo)
}

/// Compute the structural hash of a `CstNode` subtree using a shared memo.
///
/// Pass the same `memo` through all calls to `convert()` in a single
/// `process_impl` invocation so that each node's hash is computed only once
/// (O(n) SHA-256 operations instead of O(n × depth)).
pub fn structural_hash_with_memo(node: &CstNode, memo: &mut HashMap<usize, String>) -> String {
    let key = node as *const CstNode as usize;
    if let Some(cached) = memo.get(&key) {
        return cached.clone();
    }

    let payload = if node.is_leaf() {
        format!("{}:{}", node.node_type, node.text_or_empty())
    } else {
        let child_hashes: Vec<String> = node
            .children
            .iter()
            .map(|c| structural_hash_with_memo(c, memo))
            .collect();
        format!("{}|{}", node.node_type, child_hashes.join("|"))
    };

    let mut hasher = Sha256::new();
    hasher.update(payload.as_bytes());
    let result = hex::encode(hasher.finalize());
    memo.insert(key, result.clone());
    result
}

/// Extract all nodes whose types match any entry in `error_types`.
///
/// The default error type for tree-sitter is `"ERROR"`.  Call this on the
/// root CST node to surface parse errors to the host.
pub fn extract_parse_errors<'a>(root: &'a CstNode, error_types: &[&str]) -> Vec<&'a CstNode> {
    root.walk()
        .filter(|n| error_types.contains(&n.node_type.as_str()))
        .collect()
}
