//! Shared tree-sitter → CST conversion (issue #47).
//!
//! One implementation replaces the 28+ per-crate template copies that drifted until
//! class-wide fixes required sweeps (the literal-visibility fix patched 21 crates and had
//! to skip 40 variants). Parser crates keep only their language-specific data (semantic
//! type lists) and genuine label overrides; conversion mechanics live here.
//!
//! DRY is a hard rule (maintainer ruling 2026-07-06): if you are about to copy a hunk of
//! conversion logic into a second crate, hoist it here instead.

use crate::cst::CstNode;
use crate::hash::structural_hash_with_memo;
use crate::tree::{SemanticNode, SemanticNodeBuilder};

/// Literal-ish CST kinds whose source text must survive conversion even when the node has
/// named children (string/char/number innards are often unnamed tokens; dropping them made
/// literal edits hash style-only — issues #16/#21/#23/#41/#46).
pub fn cst_literal_kind(kind: &str) -> bool {
    // TEXT literals only: kinds whose value is their source text. COMPOSITE literal
    // containers (go's literal_value, composite_literal, list/map literals) carry their
    // content in NAMED CHILDREN — whole-text labels on them made a struct-literal FIELD
    // REORDER surface as a modification where the truthiness contract expects zero
    // (test_go_struct_literal_field_reorder). The bare contains("literal") clause was the
    // over-match; string/char/rune/number families and sequel's exact leaf kind remain.
    kind.contains("string")
        || kind.contains("char")
        || kind.contains("rune")
        || kind.contains("number")
        || kind.contains("integer")
        || kind.contains("float")
        || kind == "literal"
}

/// The shared tree-sitter → CstNode converter: named children only, 0-based positions,
/// text captured for leaves and literal-ish kinds (4096-char cap).
pub fn node_to_cst(node: tree_sitter::Node<'_>, source: &[u8]) -> CstNode {
    let children: Vec<CstNode> = (0..node.named_child_count())
        .filter_map(|i| node.named_child(i))
        .map(|child| node_to_cst(child, source))
        .collect();

    let keep_text = children.is_empty() || cst_literal_kind(node.kind());
    let text = if keep_text {
        Some(
            node.utf8_text(source)
                .unwrap_or("")
                .chars()
                .take(4096)
                .collect(),
        )
    } else {
        None
    };

    CstNode {
        node_type: node.kind().to_string(),
        named: node.is_named(),
        text,
        start_line: node.start_position().row as u32,
        start_col: node.start_position().column as u32,
        end_line: node.end_position().row as u32,
        end_col: node.end_position().column as u32,
        children,
    }
}

/// The shared CST → SemanticNode conversion (issue #47 tranche 2). One body replaces the
/// per-crate `fn convert` template: keep a node when its type is semantic OR semantic
/// children survived beneath it; ids are position paths under `id_prefix`; hashes come from
/// `structural_hash_with_memo`. Crates supply only DATA and genuine language behavior:
/// `is_semantic` (their `SEMANTIC_TYPES` membership) and `label_for` — a closure, so
/// span-labeling languages capture `source` in it (see make-parser) with no extra
/// parameter threading.
pub fn convert_semantic(
    node: &CstNode,
    id_prefix: &str,
    memo: &mut std::collections::HashMap<usize, String>,
    is_semantic: &dyn Fn(&str) -> bool,
    label_for: &dyn Fn(&CstNode) -> String,
) -> Option<SemanticNode> {
    let children: Vec<SemanticNode> = node
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            convert_semantic(c, &format!("{}.{}", id_prefix, i), memo, is_semantic, label_for)
        })
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

/// Hooks for the DIRECT tree-sitter → SemanticNode template (issue #47 tranche 2,
/// family 2: the parent_class-threading crates — assemblyscript, dart, squirrel,
/// clojure, r, dockerfile). This family iterates ALL children (named and anonymous),
/// labels from the raw tree, emits hash "" (hashing happens host-side), and threads a
/// class label down so method-like nodes get `parent_type` (PULL_UP/PUSH_DOWN). Every
/// genuine behavior difference is a hook; recursion, position-path ids, 0-based
/// positions and the parent_type mechanics are shared.
pub struct TsDirectHooks<'a> {
    /// Trivia kinds dropped outright (with their subtrees).
    pub is_trivia: &'a dyn Fn(&str) -> bool,
    /// The class label this node contributes to its DESCENDANTS' parent_type
    /// (None = pass the inherited one through unchanged).
    pub class_label: &'a dyn Fn(tree_sitter::Node<'_>, &[u8]) -> Option<String>,
    /// Keep a node whose semantic children all filtered out? (childful nodes are
    /// always kept unless unwrapped).
    pub keep_childless: &'a dyn Fn(tree_sitter::Node<'_>) -> bool,
    /// Replace this node by its single converted child (assemblyscript's
    /// export_statement); receives the converted-children count.
    pub unwrap_single: &'a dyn Fn(tree_sitter::Node<'_>, usize) -> bool,
    /// The node's display label.
    pub label: &'a dyn Fn(tree_sitter::Node<'_>, &[u8]) -> String,
    /// Nodes that take `parent_type` from the inherited class label.
    pub is_method_like: &'a dyn Fn(tree_sitter::Node<'_>) -> bool,
}

/// The shared body for the direct family. Call with `parent_class = None` at the root.
pub fn convert_ts_direct(
    node: tree_sitter::Node<'_>,
    source: &[u8],
    id_prefix: &str,
    parent_class: Option<&str>,
    hooks: &TsDirectHooks<'_>,
) -> Option<SemanticNode> {
    if (hooks.is_trivia)(node.kind()) {
        return None;
    }

    let child_parent_class = (hooks.class_label)(node, source)
        .or_else(|| parent_class.map(|s| s.to_string()));

    let children: Vec<SemanticNode> = (0..node.child_count())
        .filter_map(|i| {
            convert_ts_direct(
                node.child(i)?,
                source,
                &format!("{}.{}", id_prefix, i),
                child_parent_class.as_deref(),
                hooks,
            )
        })
        .collect();

    if (hooks.unwrap_single)(node, children.len()) && children.len() == 1 {
        return children.into_iter().next();
    }

    if children.is_empty() && !(hooks.keep_childless)(node) {
        return None;
    }

    let mut builder = SemanticNodeBuilder::new(
        id_prefix,
        node.kind(),
        (hooks.label)(node, source),
        node.start_position().row as u32,
        node.start_position().column as u32,
        node.end_position().row as u32,
        node.end_position().column as u32,
        "",
    )
    .children(children);

    if (hooks.is_method_like)(node) {
        if let Some(class_name) = parent_class {
            builder = builder.parent_type(class_name);
        }
    }

    Some(builder.build())
}

/// Family 3 (issue #47): the CstNode template WITH parent_class threading — kind-based
/// class/method predicates; method-like nodes get `parent_type` from the ENCLOSING class
/// label (PULL_UP/PUSH_DOWN). Shared verbatim by cpp/csharp/go/java/php/ruby/rust.
/// Call with `parent_class = None` at the root.
#[allow(clippy::too_many_arguments)]
pub fn convert_semantic_classed(
    node: &CstNode,
    id_prefix: &str,
    parent_class: Option<&str>,
    memo: &mut std::collections::HashMap<usize, String>,
    is_trivia: &dyn Fn(&str) -> bool,
    is_semantic: &dyn Fn(&str) -> bool,
    is_class_like: &dyn Fn(&str) -> bool,
    is_method_like: &dyn Fn(&str) -> bool,
    label_for: &dyn Fn(&CstNode) -> String,
) -> Option<SemanticNode> {
    if is_trivia(&node.node_type) {
        return None;
    }
    let own_class_label: Option<String> = if is_class_like(&node.node_type) {
        Some(label_for(node))
    } else {
        None
    };
    let child_parent_class: Option<&str> = own_class_label.as_deref().or(parent_class);

    let children: Vec<SemanticNode> = node
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            convert_semantic_classed(
                c,
                &format!("{}.{}", id_prefix, i),
                child_parent_class,
                memo,
                is_trivia,
                is_semantic,
                is_class_like,
                is_method_like,
                label_for,
            )
        })
        .collect();
    if !is_semantic(&node.node_type) && children.is_empty() {
        return None;
    }

    let hash = structural_hash_with_memo(node, memo);
    let mut builder = SemanticNodeBuilder::new(
        id_prefix,
        &node.node_type,
        label_for(node),
        node.start_line,
        node.start_col,
        node.end_line,
        node.end_col,
        hash,
    )
    .children(children);

    if is_method_like(&node.node_type) {
        if let Some(class_name) = parent_class {
            builder = builder.parent_type(class_name);
        }
    }

    Some(builder.build())
}

/// Family 5 (issue #47): the STRICT filter — a non-semantic node prunes its WHOLE
/// subtree (unlike convert_semantic's keep-if-children-survived rule). Shared by
/// css/html/scss, whose semantic sets enumerate every level they keep.
pub fn convert_semantic_strict(
    node: &CstNode,
    id_prefix: &str,
    memo: &mut std::collections::HashMap<usize, String>,
    is_trivia: &dyn Fn(&str) -> bool,
    is_semantic: &dyn Fn(&str) -> bool,
    label_for: &dyn Fn(&CstNode) -> String,
) -> Option<SemanticNode> {
    if is_trivia(&node.node_type) || !is_semantic(&node.node_type) {
        return None;
    }
    let children: Vec<SemanticNode> = node
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            convert_semantic_strict(
                c,
                &format!("{}.{}", id_prefix, i),
                memo,
                is_trivia,
                is_semantic,
                label_for,
            )
        })
        .collect();
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

/// Generic label for literal containers: their captured source text (120-char cap).
/// Call at the top of a crate's `label_for` before language-specific branches.
pub fn literal_label(node: &CstNode) -> Option<String> {
    if !node.is_leaf() && cst_literal_kind(&node.node_type) {
        let text = node.text_or_empty();
        if !text.is_empty() {
            return Some(text.chars().take(120).collect());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::cst_literal_kind;

    #[test]
    fn literal_kinds_cover_the_burned_families() {
        for kind in [
            "interpreted_string_literal", // go (#46)
            "decimal_integer_literal",    // java/dart (#21)
            "string_literal",
            "raw_string_literal",
            "char_literal",
            "rune_literal",
            "number",
            "literal",                     // sequel (#16)
        ] {
            assert!(cst_literal_kind(kind), "{kind} must keep text");
        }
        assert!(!cst_literal_kind("function_declaration"));
        assert!(!cst_literal_kind("block"));
    }
}
