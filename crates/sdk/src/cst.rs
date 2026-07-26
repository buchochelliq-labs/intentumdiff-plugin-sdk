//! CST node types — mirror of the JSON schema produced by the Python host's
//! `cst_serializer`.

use serde::{Deserialize, Serialize};

/// A single node from the tree-sitter CST, as deserialised from the JSON
/// string passed to the plugin's `process` function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CstNode {
    /// tree-sitter node type (e.g. `"function_definition"`, `"identifier"`)
    #[serde(rename = "type")]
    pub node_type: String,

    /// `true` for named nodes, `false` for anonymous/punctuation nodes.
    pub named: bool,

    /// Raw source text — only present for leaf nodes (no children).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// 0-based start line.
    pub start_line: u32,
    /// 0-based start column.
    pub start_col: u32,
    /// 0-based end line.
    pub end_line: u32,
    /// 0-based end column.
    pub end_col: u32,

    /// Child nodes — absent for leaves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<CstNode>,
}

impl CstNode {
    /// Return `true` if this node has no children (is a leaf).
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Return the text of a leaf node, or an empty string for internal nodes.
    pub fn text_or_empty(&self) -> &str {
        self.text.as_deref().unwrap_or("")
    }

    /// Iterate over all descendants in pre-order (self-inclusive).
    pub fn walk(&self) -> impl Iterator<Item = &CstNode> {
        // Collect into a Vec for simplicity; plugins are not performance-critical.
        let mut stack: Vec<&CstNode> = vec![self];
        let mut result: Vec<&CstNode> = Vec::new();
        while let Some(node) = stack.pop() {
            result.push(node);
            // Push children in reverse order so leftmost is visited first.
            for child in node.children.iter().rev() {
                stack.push(child);
            }
        }
        result.into_iter()
    }

    /// Parse a CST from a JSON string (as sent by the host).
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Discriminated node kind — useful for pattern-matching in plugins.
pub enum CstNodeKind<'a> {
    Leaf { text: &'a str },
    Internal { children: &'a [CstNode] },
}

impl<'a> From<&'a CstNode> for CstNodeKind<'a> {
    fn from(node: &'a CstNode) -> Self {
        if node.is_leaf() {
            CstNodeKind::Leaf {
                text: node.text_or_empty(),
            }
        } else {
            CstNodeKind::Internal {
                children: &node.children,
            }
        }
    }
}
