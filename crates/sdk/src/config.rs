//! Complete typed configuration subtrees for domain-specific parser adapters.
//!
//! Adapters keep their domain nodes and append one subtree for each property not
//! already represented. They supply decoded strings, exact numeric source atoms
//! and source positions. This module does not parse JSON/YAML, round numbers or
//! derive facts from source text. Mapping order is insignificant; sequence order
//! is significant. Unsupported syntax can remain an exact opaque source atom.

use crate::tree::{Position, SemanticNode, SemanticNodeBuilder};
use std::fmt;

/// A syntax adapter's value and exact 0-based UTF-8 byte source position.
#[derive(Debug, Clone)]
pub struct ConfigValue {
    pub position: Position,
    pub kind: ConfigKind,
}

/// A mapping field's separately anchored source key and typed value.
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    pub key: String,
    /// The exact key token, or a complete property span containing that key.
    pub key_position: Position,
    pub value: ConfigValue,
}

/// Typed payload, independent of any particular configuration syntax.
#[derive(Debug, Clone)]
pub enum ConfigKind {
    Null,
    Boolean(bool),
    /// The original numeric token, never a formatted or rounded `f64` value.
    /// Numeric spelling differences conservatively remain distinguishable.
    Number(String),
    /// Decoded runtime string value; quotes/escape spelling are not payload.
    String(String),
    Sequence(Vec<ConfigValue>),
    /// Decoded string keys. Duplicate keys decline projection rather than
    /// silently sorting away source-order-dependent duplicate semantics.
    Mapping(Vec<ConfigEntry>),
    /// Complete source for syntax the adapter cannot safely classify.
    Opaque(String),
}

/// A projection is all-or-nothing; callers must report failures explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigProjectionError {
    EmptyIdPrefix,
    EmptyNumber,
    InvalidPosition,
    DuplicateKey,
    DepthLimit,
    NodeLimit,
}

impl fmt::Display for ConfigProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyIdPrefix => "configuration node ID prefix must not be empty",
            Self::EmptyNumber => "configuration numeric source atom must not be empty",
            Self::InvalidPosition => "configuration source position is outside UTF-8 source bounds",
            Self::DuplicateKey => "duplicate configuration mapping key has unproven semantics",
            Self::DepthLimit => "configuration projection depth limit exceeded",
            Self::NodeLimit => "configuration projection node limit exceeded",
        })
    }
}

impl std::error::Error for ConfigProjectionError {}

const MAX_DEPTH: usize = 128;
const MAX_NODES: usize = 100_000;

/// Project one omitted domain property using the existing semantic node builder.
///
/// `id_prefix` must be unique among the adapter's other domain/property nodes.
/// IDs do not interpolate keys, so keys containing dots, slashes or separators
/// cannot collide. Positions are validated against `source`, including UTF-8
/// boundaries. A parent's semantic anchor need not contain all child anchors.
/// The helper deliberately supplies no `NodeFacts` and uses the SDK's existing
/// label-aware builder hashing, not a parallel identity algorithm.
pub fn configuration_node(
    id_prefix: &str,
    field_name: &str,
    key_position: &Position,
    value: &ConfigValue,
    source: &str,
) -> Result<SemanticNode, ConfigProjectionError> {
    ConfigurationProjector::new(source).configuration_node(
        id_prefix,
        field_name,
        key_position,
        value,
    )
}

/// A reusable UTF-8 source index for all omitted properties in one parse.
///
/// Create this once per source and call [`Self::configuration_node`] for each
/// omitted property. Reusing the index avoids scanning a large source once per
/// property. ID prefixes remain the caller's domain-specific responsibility.
pub struct ConfigurationProjector<'source> {
    lines: Vec<&'source str>,
}

impl<'source> ConfigurationProjector<'source> {
    pub fn new(source: &'source str) -> Self {
        Self {
            lines: source.split('\n').collect(),
        }
    }

    pub fn configuration_node(
        &self,
        id_prefix: &str,
        field_name: &str,
        key_position: &Position,
        value: &ConfigValue,
    ) -> Result<SemanticNode, ConfigProjectionError> {
        if id_prefix.is_empty() {
            return Err(ConfigProjectionError::EmptyIdPrefix);
        }
        let mut projector = Projector {
            id_prefix,
            next_id: 0,
            lines: &self.lines,
        };
        projector.position(key_position)?;
        projector.position(&value.position)?;
        let id = projector.id()?;
        let child = projector.value(value, 0)?;
        Ok(build(id, "property", field_name, key_position, vec![child]))
    }
}

struct Projector<'a, 'source> {
    id_prefix: &'a str,
    next_id: usize,
    lines: &'a [&'source str],
}

impl Projector<'_, '_> {
    fn id(&mut self) -> Result<String, ConfigProjectionError> {
        if self.next_id >= MAX_NODES {
            return Err(ConfigProjectionError::NodeLimit);
        }
        let id = format!("{}:{}", self.id_prefix, self.next_id);
        self.next_id += 1;
        Ok(id)
    }

    fn position(&self, p: &Position) -> Result<(), ConfigProjectionError> {
        if (p.start_line, p.start_col) > (p.end_line, p.end_col) {
            return Err(ConfigProjectionError::InvalidPosition);
        }
        for (line, col) in [(p.start_line, p.start_col), (p.end_line, p.end_col)] {
            let Some(text) = self.lines.get(line as usize) else {
                return Err(ConfigProjectionError::InvalidPosition);
            };
            if !text.is_char_boundary(col as usize) {
                return Err(ConfigProjectionError::InvalidPosition);
            }
        }
        Ok(())
    }

    fn value(
        &mut self,
        value: &ConfigValue,
        depth: usize,
    ) -> Result<SemanticNode, ConfigProjectionError> {
        if depth > MAX_DEPTH {
            return Err(ConfigProjectionError::DepthLimit);
        }
        self.position(&value.position)?;
        let id = self.id()?;
        let mut children = Vec::new();
        let (kind, label) = match &value.kind {
            ConfigKind::Null => ("null", "null"),
            ConfigKind::Boolean(value) => ("boolean", if *value { "true" } else { "false" }),
            ConfigKind::Number(value) => {
                if value.is_empty() {
                    return Err(ConfigProjectionError::EmptyNumber);
                }
                ("number", value.as_str())
            }
            ConfigKind::String(value) => ("string", value.as_str()),
            ConfigKind::Opaque(value) => ("value", value.as_str()),
            ConfigKind::Sequence(values) => {
                for value in values {
                    children.push(self.value(value, depth + 1)?);
                }
                ("array", "")
            }
            ConfigKind::Mapping(entries) => {
                let mut sorted: Vec<_> = entries.iter().collect();
                sorted.sort_unstable_by(|a, b| a.key.cmp(&b.key));
                if sorted.windows(2).any(|pair| pair[0].key == pair[1].key) {
                    return Err(ConfigProjectionError::DuplicateKey);
                }
                for entry in sorted {
                    self.position(&entry.key_position)?;
                    let property_id = self.id()?;
                    children.push(build(
                        property_id,
                        "property",
                        &entry.key,
                        &entry.key_position,
                        vec![self.value(&entry.value, depth + 1)?],
                    ));
                }
                ("object", "")
            }
        };
        Ok(build(id, kind, label, &value.position, children))
    }
}

fn build(
    id: String,
    kind: &str,
    label: &str,
    p: &Position,
    children: Vec<SemanticNode>,
) -> SemanticNode {
    SemanticNodeBuilder::new(
        id,
        kind,
        label,
        p.start_line,
        p.start_col,
        p.end_line,
        p.end_col,
        String::new(),
    )
    .children(children)
    .build()
}
