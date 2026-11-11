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
use crate::hash::{structural_hash_with_options, HashOptions};
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
/// complete source text captured for leaves and literal-ish kinds. This text is hash
/// input, so shortening it would hide edits past the truncation boundary.
///
/// Equivalent to [`node_to_cst_with`] with [`CstOptions::default()`] (the certified
/// behavior). Anonymous operator/keyword tokens (`+`/`-`, `async`, `:=`, …) are DROPPED
/// here; opt in with [`CstOptions`] to keep them (issue #25).
pub fn node_to_cst(node: tree_sitter::Node<'_>, source: &[u8]) -> CstNode {
    node_to_cst_with(node, source, &CstOptions::default())
}

/// Which anonymous (unnamed) tree-sitter children [`node_to_cst_with`] keeps.
#[derive(Clone, Copy, Debug, Default)]
pub enum AnonymousTokens {
    /// Legacy: named children only. Operator/keyword tokens vanish from the CST, so
    /// `a + b` and `a - b` (or `def`/`async def`) hash identically.
    #[default]
    Drop,
    /// Keep anonymous tokens except pure delimiters/punctuation
    /// ([`is_delimiter_token`]): operators (`+`, `<=`, `:=`, `->`, `?.`) and keywords
    /// (`async`, `virtual`, `not`, `-eq`) survive; `(`, `,`, `;` … do not.
    OperatorsAndKeywords,
    /// Keep every anonymous token, delimiters included.
    All,
    /// Keep the anonymous tokens whose kind matches the predicate.
    Custom(fn(&str) -> bool),
}

impl AnonymousTokens {
    /// Whether an anonymous child of kind `kind` is kept.
    pub fn keeps(&self, kind: &str) -> bool {
        match self {
            AnonymousTokens::Drop => false,
            AnonymousTokens::OperatorsAndKeywords => {
                !kind.trim().is_empty() && !is_delimiter_token(kind)
            }
            AnonymousTokens::All => true,
            AnonymousTokens::Custom(keep) => keep(kind),
        }
    }
}

/// Options for [`node_to_cst_with`]. `CstOptions::default()` is the legacy converter.
#[derive(Clone, Copy, Debug, Default)]
pub struct CstOptions {
    /// Which anonymous tokens become CST leaves.
    pub anonymous_tokens: AnonymousTokens,
}

impl CstOptions {
    /// Recommended for new adopters: keep operator and keyword tokens.
    pub const SEMANTIC_TOKENS: CstOptions = CstOptions {
        anonymous_tokens: AnonymousTokens::OperatorsAndKeywords,
    };
}

/// Pure punctuation/delimiter token kinds that carry no meaning on their own: brackets,
/// separators, member dots, type/label colons and quote marks. Formatting can't change them
/// without also changing named structure, so [`AnonymousTokens::OperatorsAndKeywords`]
/// leaves them out of hashes.
pub fn is_delimiter_token(kind: &str) -> bool {
    matches!(
        kind,
        "(" | ")"
            | "["
            | "]"
            | "{"
            | "}"
            | ","
            | ";"
            | "."
            | ":"
            | "::"
            | "\""
            | "'"
            | "`"
            | "\"\"\""
            | "\'\'\'"
            | "\n"
            | "\r\n"
    )
}

/// [`node_to_cst`] with explicit [`CstOptions`]. A kept anonymous token becomes a leaf
/// with `named: false`, its real span, and `text` = its grammar token kind (for
/// case-insensitive grammars `SELECT`/`select` stay one token). Anonymous tokens that
/// themselves have children (rare aliases) are converted like named nodes.
///
/// Kept tokens shift CST child indices, so position-path ids and any `label_for` that
/// indexes `children` positionally change: filter on `named` when picking names.
pub fn node_to_cst_with(node: tree_sitter::Node<'_>, source: &[u8], options: &CstOptions) -> CstNode {
    let children: Vec<CstNode> = match options.anonymous_tokens {
        AnonymousTokens::Drop => (0..node.named_child_count())
            .filter_map(|i| node.named_child(i))
            .map(|child| node_to_cst_with(child, source, options))
            .collect(),
        tokens => (0..node.child_count())
            .filter_map(|i| node.child(i))
            .filter(|child| child.is_named() || tokens.keeps(child.kind()))
            .map(|child| node_to_cst_with(child, source, options))
            .collect(),
    };

    let keep_text = children.is_empty() || cst_literal_kind(node.kind());
    let text = if !keep_text {
        None
    } else if !node.is_named() && children.is_empty() {
        Some(node.kind().to_string())
    } else {
        Some(node.utf8_text(source).unwrap_or("").to_string())
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
/// `structural_hash_with_memo` (see [`convert_semantic_with`] for opt-in identity hashing). Crates supply only DATA and genuine language behavior:
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
    convert_semantic_with(node, id_prefix, memo, is_semantic, label_for, &HashOptions::default())
}

/// [`convert_semantic`] hashing with explicit [`HashOptions`] (issue #25; pass
/// [`HashOptions::IDENTITY`] to hash literal payload and exclude comments).
pub fn convert_semantic_with(
    node: &CstNode,
    id_prefix: &str,
    memo: &mut std::collections::HashMap<usize, String>,
    is_semantic: &dyn Fn(&str) -> bool,
    label_for: &dyn Fn(&CstNode) -> String,
    hash_options: &HashOptions<'_>,
) -> Option<SemanticNode> {
    let children: Vec<SemanticNode> = node
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            convert_semantic_with(
                c,
                &format!("{}.{}", id_prefix, i),
                memo,
                is_semantic,
                label_for,
                hash_options,
            )
        })
        .collect();
    if !is_semantic(&node.node_type) && children.is_empty() {
        return None;
    }
    let hash = structural_hash_with_options(node, memo, hash_options);
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
    convert_semantic_classed_with(
        node,
        id_prefix,
        parent_class,
        memo,
        is_trivia,
        is_semantic,
        is_class_like,
        is_method_like,
        label_for,
        &HashOptions::default(),
    )
}

/// [`convert_semantic_classed`] hashing with explicit [`HashOptions`] (issue #25).
#[allow(clippy::too_many_arguments)]
pub fn convert_semantic_classed_with(
    node: &CstNode,
    id_prefix: &str,
    parent_class: Option<&str>,
    memo: &mut std::collections::HashMap<usize, String>,
    is_trivia: &dyn Fn(&str) -> bool,
    is_semantic: &dyn Fn(&str) -> bool,
    is_class_like: &dyn Fn(&str) -> bool,
    is_method_like: &dyn Fn(&str) -> bool,
    label_for: &dyn Fn(&CstNode) -> String,
    hash_options: &HashOptions<'_>,
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
            convert_semantic_classed_with(
                c,
                &format!("{}.{}", id_prefix, i),
                child_parent_class,
                memo,
                is_trivia,
                is_semantic,
                is_class_like,
                is_method_like,
                label_for,
                hash_options,
            )
        })
        .collect();
    if !is_semantic(&node.node_type) && children.is_empty() {
        return None;
    }

    let hash = structural_hash_with_options(node, memo, hash_options);
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
    convert_semantic_strict_with(
        node,
        id_prefix,
        memo,
        is_trivia,
        is_semantic,
        label_for,
        &HashOptions::default(),
    )
}

/// [`convert_semantic_strict`] hashing with explicit [`HashOptions`] (issue #25).
pub fn convert_semantic_strict_with(
    node: &CstNode,
    id_prefix: &str,
    memo: &mut std::collections::HashMap<usize, String>,
    is_trivia: &dyn Fn(&str) -> bool,
    is_semantic: &dyn Fn(&str) -> bool,
    label_for: &dyn Fn(&CstNode) -> String,
    hash_options: &HashOptions<'_>,
) -> Option<SemanticNode> {
    if is_trivia(&node.node_type) || !is_semantic(&node.node_type) {
        return None;
    }
    let children: Vec<SemanticNode> = node
        .children
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            convert_semantic_strict_with(
                c,
                &format!("{}.{}", id_prefix, i),
                memo,
                is_trivia,
                is_semantic,
                label_for,
                hash_options,
            )
        })
        .collect();
    let hash = structural_hash_with_options(node, memo, hash_options);
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

/// Display cap (in chars) applied by [`literal_label`].
pub const LITERAL_LABEL_MAX_CHARS: usize = 120;

/// Generic label for literal containers: their captured source text (120-char cap).
/// Call at the top of a crate's `label_for` before language-specific branches.
///
/// The cap is display-only; when labels feed identity (builder-computed hashes, host
/// matching on labels) use [`literal_label_full`] (issue #25).
pub fn literal_label(node: &CstNode) -> Option<String> {
    literal_label_full(node).map(|text| text.chars().take(LITERAL_LABEL_MAX_CHARS).collect())
}

/// [`literal_label`] without the display cap: the complete retained literal text.
pub fn literal_label_full(node: &CstNode) -> Option<String> {
    if !node.is_leaf() && cst_literal_kind(&node.node_type) {
        let text = node.text_or_empty();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        convert_semantic, convert_semantic_with, cst_literal_kind, is_delimiter_token,
        literal_label, literal_label_full, node_to_cst, node_to_cst_with, AnonymousTokens,
        CstOptions,
    };
    use crate::{
        cst::CstNode,
        hash::{structural_hash, structural_hash_with_options, HashOptions},
    };
    use std::collections::HashMap;

    fn python_cst_with(source: &str, options: &CstOptions) -> CstNode {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        assert!(!tree.root_node().has_error());
        node_to_cst_with(tree.root_node(), source.as_bytes(), options)
    }

    const SEMANTIC_TOKENS: CstOptions = CstOptions {
        anonymous_tokens: AnonymousTokens::OperatorsAndKeywords,
    };
    const ALL_TOKENS: CstOptions = CstOptions {
        anonymous_tokens: AnonymousTokens::All,
    };

    fn identity(node: &CstNode) -> String {
        structural_hash_with_options(node, &mut HashMap::new(), &HashOptions::IDENTITY)
    }

    #[test]
    fn default_options_reproduce_the_named_only_converter() {
        for source in ["x = a + b\n", "async def f():\n    return not x\n", "s = 'a\\n'  # c\n"] {
            let legacy = python_cst(source);
            let default = python_cst_with(source, &CstOptions::default());
            assert_eq!(
                serde_json::to_value(&legacy).unwrap(),
                serde_json::to_value(&default).unwrap()
            );
        }
    }

    #[test]
    fn operator_and_keyword_tokens_survive_when_opted_in() {
        let cases = [
            ("x = a + b\n", "x = a - b\n"),
            ("def f():\n    pass\n", "async def f():\n    pass\n"),
            ("y = x\n", "y = not x\n"),
            ("y = a < b\n", "y = a <= b\n"),
            ("y = a and b\n", "y = a or b\n"),
        ];
        for (old, new) in cases {
            // The legacy converter loses the token entirely (for `not`, the unary node
            // itself survives, so only the binary cases collide).
            let opted_old = python_cst_with(old, &SEMANTIC_TOKENS);
            let opted_new = python_cst_with(new, &SEMANTIC_TOKENS);
            assert_ne!(
                structural_hash(&opted_old),
                structural_hash(&opted_new),
                "{old:?} -> {new:?} must change the hash"
            );
        }
        assert_eq!(
            structural_hash(&python_cst("x = a + b\n")),
            structural_hash(&python_cst("x = a - b\n")),
            "documents the legacy named-only loss"
        );
    }

    #[test]
    fn delimiters_are_dropped_unless_all_tokens_are_kept() {
        let source = "f(a, b)\n";
        let semantic = python_cst_with(source, &SEMANTIC_TOKENS);
        let all = python_cst_with(source, &ALL_TOKENS);
        assert!(!semantic.walk().any(|n| n.node_type == "(" || n.node_type == ","));
        assert!(all.walk().any(|n| n.node_type == "(" && !n.named));
        assert!(all.walk().any(|n| n.node_type == ","));
        // Formatting-only change stays hash-neutral with semantic tokens.
        assert_eq!(
            structural_hash(&semantic),
            structural_hash(&python_cst_with("f(a,b)\n", &SEMANTIC_TOKENS))
        );
        let custom = CstOptions {
            anonymous_tokens: AnonymousTokens::Custom(|kind| kind == "("),
        };
        let custom_cst = python_cst_with(source, &custom);
        assert!(custom_cst.walk().any(|n| n.node_type == "("));
        assert!(!custom_cst.walk().any(|n| n.node_type == ","));
        for d in ["(", ")", "[", "]", "{", "}", ",", ";", ".", ":", "\"", "'", "`", "\"\"\""] {
            assert!(is_delimiter_token(d), "{d}");
        }
        for t in ["+", "-", ":=", "->", "async", "virtual", "-eq", "?.", "..", "**"] {
            assert!(!is_delimiter_token(t), "{t}");
        }
    }

    #[test]
    fn kept_tokens_are_anonymous_leaves_with_valid_spans() {
        let source = "x = a + b\n";
        let cst = python_cst_with(source, &SEMANTIC_TOKENS);
        let plus = cst.walk().find(|n| n.node_type == "+").expect("+ kept");
        assert!(!plus.named);
        assert!(plus.is_leaf());
        assert_eq!(plus.text.as_deref(), Some("+"));
        assert_eq!((plus.start_line, plus.start_col, plus.end_col), (0, 6, 7));
        let eq = cst.walk().find(|n| n.node_type == "=").expect("= kept");
        assert_eq!((eq.start_col, eq.end_col), (2, 3));
    }

    #[test]
    fn identity_hash_ignores_comments_but_not_code() {
        let a = python_cst_with("x = 1  # one\n", &SEMANTIC_TOKENS);
        let b = python_cst_with("x = 1  # two\n", &SEMANTIC_TOKENS);
        let c = python_cst_with("x = 2  # one\n", &SEMANTIC_TOKENS);
        assert_ne!(structural_hash(&a), structural_hash(&b));
        assert_eq!(identity(&a), identity(&b));
        assert_ne!(identity(&a), identity(&c));
    }

    #[test]
    fn literal_labels_full_text_is_opt_in() {
        let payload = "a".repeat(300);
        let cst = python_cst(&format!("v = \"{payload}X\"\n"));
        let string = cst.walk().find(|n| n.node_type == "string").unwrap();
        assert_eq!(literal_label(string).unwrap().chars().count(), 120);
        assert_eq!(literal_label_full(string).unwrap(), format!("\"{payload}X\""));
    }

    #[test]
    fn converters_with_identity_options_hash_literal_payload() {
        fn literal(text: &str) -> CstNode {
            CstNode {
                node_type: "string_literal".into(),
                named: true,
                text: Some(text.into()),
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: text.len() as u32,
                children: vec![CstNode {
                    node_type: "escape_sequence".into(),
                    named: true,
                    text: Some("\\n".into()),
                    start_line: 0,
                    start_col: 4,
                    end_line: 0,
                    end_col: 6,
                    children: vec![],
                }],
            }
        }
        let is_semantic = |kind: &str| kind == "string_literal";
        let label = |n: &CstNode| literal_label_full(n).unwrap_or_default();
        let run = |n: &CstNode, opts: &HashOptions| {
            convert_semantic_with(n, "0", &mut HashMap::new(), &is_semantic, &label, opts)
                .unwrap()
                .structural_hash
        };
        let foo = literal("\"foo\\n\"");
        let bar = literal("\"bar\\n\"");
        assert_ne!(run(&foo, &HashOptions::IDENTITY), run(&bar, &HashOptions::IDENTITY));
        // Default options == legacy converter.
        let legacy = convert_semantic(&foo, "0", &mut HashMap::new(), &is_semantic, &label)
            .unwrap()
            .structural_hash;
        assert_eq!(legacy, run(&foo, &HashOptions::default()));
        assert_eq!(legacy, run(&bar, &HashOptions::default()));
    }

    fn python_cst(source: &str) -> CstNode {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        assert!(!tree.root_node().has_error());
        node_to_cst(tree.root_node(), source.as_bytes())
    }

    #[test]
    fn node_to_cst_hashes_literal_suffixes_beyond_4096_characters() {
        for prefix_len in [4095, 4096, 4097, 4200, 16384] {
            let prefix = "a".repeat(prefix_len);
            let old = python_cst(&format!("value = \"{prefix}x\"\n"));
            let new = python_cst(&format!("value = \"{prefix}y\"\n"));
            assert_ne!(
                structural_hash(&old),
                structural_hash(&new),
                "literal suffix after {prefix_len} characters must affect its subtree hash"
            );
        }
    }

    #[test]
    fn node_to_cst_retains_complete_unicode_literal_text_and_byte_spans() {
        let payload = format!("{}x", "é🙂".repeat(2100));
        let literal = format!("\"{payload}\"");
        let source = format!("value = {literal}\n");
        let cst = python_cst(&source);
        let string = cst.walk().find(|node| node.node_type == "string").unwrap();
        let content = cst
            .walk()
            .find(|node| node.node_type == "string_content")
            .unwrap();

        assert!(
            string.text.as_deref() == Some(literal.as_str()),
            "complete literal text must survive conversion"
        );
        assert!(
            content.text.as_deref() == Some(payload.as_str()),
            "complete leaf payload must survive conversion"
        );
        assert_eq!(content.start_line, 0);
        assert_eq!(content.end_line, 0);
        assert_eq!(content.start_col, 9);
        assert_eq!(content.end_col, 9 + payload.len() as u32);
        let new = python_cst(&source.replace('x', "y"));
        assert_ne!(structural_hash(&cst), structural_hash(&new));
    }

    #[test]
    fn complete_long_literal_conversion_is_deterministic_and_spacing_preserving() {
        let payload = "a".repeat(4200);
        let source = format!("value = \"{payload}\"\n");
        let first = python_cst(&source);
        let repeated = python_cst(&source);
        let spaced = python_cst(&format!("value=\"{payload}\"\n"));

        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&repeated).unwrap()
        );
        assert_eq!(structural_hash(&first), structural_hash(&spaced));
    }

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
