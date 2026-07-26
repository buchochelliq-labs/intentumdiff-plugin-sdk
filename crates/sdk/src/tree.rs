//! SemanticNode and SemanticTree — the output types that plugins return to the
//! host as JSON.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A node in the semantic tree, produced by the plugin and consumed by the
/// Python host's GumTree engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticNode {
    /// Unique identifier within the tree — must be unique per call to
    /// `process`.  Typically constructed from the node's byte offset.
    pub id: String,

    /// Semantic node type (language-specific, e.g. `"function"`, `"class"`).
    pub node_type: String,

    /// Short human-readable label — e.g. the function name for a function node.
    pub label: String,

    /// Source position.
    pub position: Position,

    /// Structural hash — call `structural_hash()` from the SDK to compute.
    pub structural_hash: String,

    /// Child nodes (recursive).
    #[serde(default)]
    pub children: Vec<SemanticNode>,

    /// The name of the class that directly contains this node.
    /// Set by parsers for method nodes to enable PULL_UP / PUSH_DOWN detection.
    /// Omitted from JSON when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_type: Option<String>,

    /// Privacy-safe structural facts for definition nodes (functions/methods/classes),
    /// computed by the parser from the full CST before pruning. Contains only
    /// counts/enums/flags — never source text, identifiers, or literals — so it can
    /// describe a change's shape ("no parameters", "returns nothing", "no-op body")
    /// without revealing code. Omitted from JSON when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facts: Option<NodeFacts>,
}

/// Structural, privacy-safe facts about a definition node. Every field is optional
/// so a parser only emits what it can positively determine (never a guess).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeFacts {
    /// Number of declared parameters (0 for `()`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param_count: Option<u32>,

    /// `"none"` (returns nothing / None), `"value"` (returns an expression), or `"unknown"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returns: Option<String>,

    /// `"empty"`, `"stub"` (no-op: only `pass`/`...`/docstring/`raise NotImplementedError`),
    /// or `"substantive"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// `Some(true)` for `async` definitions; omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_async: Option<bool>,

    /// `Some(true)` when the body contains `yield` (a generator); omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_generator: Option<bool>,

    /// Number of decorators/annotations applied to the definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorator_count: Option<u32>,
}

/// Source position (0-based lines and columns).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Builder for constructing `SemanticNode` trees incrementally.
pub struct SemanticNodeBuilder {
    id: String,
    node_type: String,
    label: String,
    position: Position,
    structural_hash: String,
    children: Vec<SemanticNode>,
    parent_type: Option<String>,
    facts: Option<NodeFacts>,
}

impl SemanticNodeBuilder {
    pub fn new(
        id: impl Into<String>,
        node_type: impl Into<String>,
        label: impl Into<String>,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
        structural_hash: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            node_type: node_type.into(),
            label: label.into(),
            position: Position {
                start_line,
                start_col,
                end_line,
                end_col,
            },
            structural_hash: structural_hash.into(),
            children: Vec::new(),
            parent_type: None,
            facts: None,
        }
    }

    pub fn child(mut self, child: SemanticNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: Vec<SemanticNode>) -> Self {
        self.children = children;
        self
    }

    /// Set the enclosing class name for method nodes (PULL_UP / PUSH_DOWN).
    pub fn parent_type(mut self, class_name: impl Into<String>) -> Self {
        self.parent_type = Some(class_name.into());
        self
    }

    /// Attach privacy-safe structural facts (definition nodes only).
    pub fn facts(mut self, facts: NodeFacts) -> Self {
        self.facts = Some(facts);
        self
    }

    pub fn build(self) -> SemanticNode {
        let hash = if self.structural_hash.is_empty() {
            let payload = if self.children.is_empty() {
                format!("{}:{}", self.node_type, self.label)
            } else {
                let child_hashes: Vec<&str> = self
                    .children
                    .iter()
                    .map(|c| c.structural_hash.as_str())
                    .collect();
                // The node's own label MUST participate: semantic converters hoist names
                // into labels (dropping the identifier leaf), so a label-blind internal
                // hash made `Sub Greet` and `Sub GreetRenamed` structurally identical —
                // renames became STYLE-ONLY for every builder-based parser (issue #46,
                // vbnet finding).
                format!("{}:{}|{}", self.node_type, self.label, child_hashes.join("|"))
            };
            let mut hasher = Sha256::new();
            hasher.update(payload.as_bytes());
            hex::encode(hasher.finalize())
        } else {
            self.structural_hash
        };
        SemanticNode {
            id: self.id,
            node_type: self.node_type,
            label: self.label,
            position: self.position,
            structural_hash: hash,
            children: self.children,
            parent_type: self.parent_type,
            facts: self.facts,
        }
    }
}

/// The top-level result returned by `process` — serialised as JSON.
pub type SemanticTree = SemanticNode;
