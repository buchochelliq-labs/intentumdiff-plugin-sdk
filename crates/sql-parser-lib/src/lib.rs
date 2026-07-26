//! Shared SQL semantic CST utilities for SQL plugins.
//!
//! Extracted so that both the built-in `sql-parser` and the `dbt-parser`
//! (which preprocesses Jinja2 then runs the same SQL logic) can share code
//! without duplication.

use intentdiff_plugin_sdk::{
    cst::CstNode,
    hash::structural_hash_with_memo,
    tree::{SemanticNode, SemanticNodeBuilder},
};

/// CST node type names considered trivia for SQL.
pub const TRIVIA: &[&str] = &["comment", "line_comment", "block_comment", "whitespace"];

/// Semantic node types that are meaningful for diffing SQL.
pub const SEMANTIC_TYPES: &[&str] = &[
    // Real tree-sitter-sequel kinds (issue #16 kind drift: the previous list named a
    // different grammar's vocabulary — select_statement/column_reference/number etc. —
    // so everything inside a statement pruned and SELECT 1 vs SELECT 2 hashed
    // tree-identical, a false style-only).
    "statement",
    "select",
    "select_expression",
    "term",
    "literal",
    "field",
    "all_fields",
    "object_reference",
    "relation",
    "from",
    "where",
    "group_by",
    "order_by",
    "order_target",
    "limit",
    "insert",
    "update",
    "delete",
    "cte",
    "create_table",
    "create_view",
    "create_materialized_view",
    "create_index",
    "create_function",
    "function_declaration",
    "function_body",
    "alter_table",
    "drop_table",
    "column",
    "column_definition",
    "column_definitions",
    "invocation",
    "binary_expression",
    "between_expression",
    "case",
    "cast",
    "assignment",
    "assignment_list",
    "comment_statement",
    // Kept from the previous list for cross-dialect compatibility (tsql/plsql share this lib)
    "select_statement",
    "select_clause",
    "from_clause",
    "where_clause",
    "group_by_clause",
    "having_clause",
    "order_by_clause",
    "limit_clause",
    "join_clause",
    "insert_statement",
    "update_statement",
    "delete_statement",
    "create_table_statement",
    "alter_table_statement",
    "drop_table_statement",
    "create_view_statement",
    "create_index_statement",
    "function_definition",
    "procedure_definition",
    "with_clause",
    "common_table_expression",
    "subquery",
    "column_reference",
    "table_reference",
    "function_call",
    "identifier",
    "string",
    "number",
    "boolean",
    "null",
];

/// Return `true` if `node_type` is a semantically meaningful SQL node type.
pub fn is_semantic(node_type: &str) -> bool {
    SEMANTIC_TYPES.contains(&node_type)
}

/// Derive a short, human-readable label for a CST node.
///
/// * Leaf nodes use their text content (uppercased for SQL keywords).
/// * Internal nodes use the first `identifier` child's text, or the node type.
pub fn label_for(node: &CstNode) -> String {
    if node.is_leaf() {
        // Only KEYWORDS normalize to uppercase; literal/identifier content must keep its
        // exact text — 'a' vs 'A' in a string literal is a real value change (issue #16).
        if node.node_type.starts_with("keyword_") {
            return node.text_or_empty().to_uppercase();
        }
        return node.text_or_empty().to_string();
    }
    for child in &node.children {
        if child.node_type == "identifier" {
            return child.text_or_empty().to_string();
        }
    }
    node.node_type.clone()
}

/// Recursively convert a `CstNode` subtree to a `SemanticNode` subtree.
///
/// Returns `None` if the node and all its descendants are non-semantic.
pub fn convert(
    node: &CstNode,
    id_prefix: &str,
    memo: &mut std::collections::HashMap<usize, String>,
) -> Option<SemanticNode> {
    let children: Vec<SemanticNode> = node
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, c)| convert(c, &format!("{}.{}", id_prefix, i), memo))
        .collect();

    if !is_semantic(&node.node_type) && children.is_empty() {
        return None;
    }

    let hash = structural_hash_with_memo(node, memo);
    Some(
        SemanticNodeBuilder::new(
            id_prefix,
            &node.node_type,
            label_for(node),
            node.start_line,
            node.start_col,
            node.end_line,
            node.end_col,
            hash,
        )
        .children(children)
        .build(),
    )
}

/// Parse a CST JSON string (as passed by the host) into a `SemanticNode` tree.
///
/// Returns a JSON string conforming to the `SemanticTree` schema, or a JSON
/// object with an `"error"` key on failure.
pub fn process_impl(cst_json: &str) -> String {
    let root: CstNode = match serde_json::from_str(cst_json) {
        Ok(n) => n,
        Err(e) => return format!(r#"{{"error":"CST parse failed: {}"}}"#, e),
    };
    let mut memo: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    let sem = match convert(&root, "0", &mut memo) {
        Some(n) => n,
        None => return r#"{"error":"Empty SQL semantic tree"}"#.to_string(),
    };
    serde_json::to_string(&sem).unwrap_or_else(|e| format!(r#"{{"error":"Serialisation: {}"}}"#, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Guards issue #16: the kind list once named a different grammar's vocabulary, so every
    // statement's interior pruned and SELECT 1 vs SELECT 2 hashed tree-identical (false
    // style-only). These pin the real tree-sitter-sequel kinds and literal-preserving labels.
    #[test]
    fn sequel_core_kinds_are_semantic() {
        for kind in ["select", "term", "literal", "field", "object_reference", "from", "where"] {
            assert!(is_semantic(kind), "real sequel kind {kind:?} must be semantic");
        }
    }

    #[test]
    fn literal_labels_keep_exact_text_keywords_uppercase() {
        let literal = CstNode {
            node_type: "literal".to_string(),
            named: true,
            text: Some("'hello'".to_string()),
            start_line: 0,
            start_col: 7,
            end_line: 0,
            end_col: 14,
            children: vec![],
        };
        assert_eq!(label_for(&literal), "'hello'", "literal content must not be case-normalized");
        let keyword = CstNode {
            node_type: "keyword_select".to_string(),
            named: true,
            text: Some("select".to_string()),
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 6,
            children: vec![],
        };
        assert_eq!(label_for(&keyword), "SELECT");
    }
}
