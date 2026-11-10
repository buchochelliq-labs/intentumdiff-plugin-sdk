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
///
/// This is the certified legacy algorithm and equals
/// [`structural_hash_with_options`] with [`HashOptions::default()`]. New parsers should
/// prefer [`HashOptions::IDENTITY`] (issue #25).
pub fn structural_hash_with_memo(node: &CstNode, memo: &mut HashMap<usize, String>) -> String {
    structural_hash_with_options(node, memo, &HashOptions::default())
}

/// Opt-in identity rules for [`structural_hash_with_options`] (issue #25).
///
/// The default (`HashOptions::default()`) reproduces the legacy algorithm byte for byte, so
/// existing, certified parser outputs do not change until a parser opts in.
#[derive(Clone, Copy, Default)]
pub struct HashOptions<'a> {
    /// Hash the retained `text` of INTERNAL nodes too. The shared converter keeps the full
    /// source text of literal containers (`string_literal` with an `escape_sequence` child,
    /// …) but the legacy hash only looks at children, so `"foo\n"` and `"bar\n"` collide.
    /// With this set, an internal node that carries text hashes as
    /// `type ":" byte_len ":" text "|" join("|", children)`. Leaves are unchanged.
    pub literal_text: bool,
    /// Children whose `node_type` matches are left out of their parent's hash (e.g.
    /// comments), so a comment-only edit does not change container/root hashes. The node
    /// passed to the hash function is always hashed itself, even when it is trivia.
    pub is_trivia: Option<&'a dyn Fn(&str) -> bool>,
}

impl HashOptions<'static> {
    /// Recommended identity rules: literal payload is hashed, comments are trivia.
    pub const IDENTITY: HashOptions<'static> = HashOptions {
        literal_text: true,
        is_trivia: Some(&is_comment_kind),
    };
}

impl std::fmt::Debug for HashOptions<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashOptions")
            .field("literal_text", &self.literal_text)
            .field("is_trivia", &self.is_trivia.map(|_| "<fn>"))
            .finish()
    }
}

/// `true` for comment node kinds (`comment`, `line_comment`, `block_comment`,
/// `doc_comment`, …): any kind containing "comment", ASCII case-insensitively.
pub fn is_comment_kind(kind: &str) -> bool {
    kind.to_ascii_lowercase().contains("comment")
}

/// Structural hash with explicit [`HashOptions`].
///
/// The `memo` is keyed by node address, so use a separate memo per options value.
pub fn structural_hash_with_options(
    node: &CstNode,
    memo: &mut HashMap<usize, String>,
    options: &HashOptions<'_>,
) -> String {
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
            .filter(|c| !options.is_trivia.is_some_and(|is_trivia| is_trivia(&c.node_type)))
            .map(|c| structural_hash_with_options(c, memo, options))
            .collect();
        match node.text.as_deref() {
            Some(text) if options.literal_text => format!(
                "{}:{}:{}|{}",
                node.node_type,
                text.len(),
                text,
                child_hashes.join("|")
            ),
            _ => format!("{}|{}", node.node_type, child_hashes.join("|")),
        }
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
