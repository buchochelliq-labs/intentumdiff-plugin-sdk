use intentumdiff_plugin_sdk::config::{
    configuration_node, ConfigEntry, ConfigKind, ConfigProjectionError, ConfigValue,
    ConfigurationProjector,
};
use intentumdiff_plugin_sdk::tree::{Position, SemanticNode};
use std::collections::HashSet;

fn value(kind: ConfigKind, source: &str) -> ConfigValue {
    ConfigValue {
        position: Position {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: source.len() as u32,
        },
        kind,
    }
}

fn node(kind: ConfigKind, source: &str) -> SemanticNode {
    let v = value(kind, source);
    configuration_node("domain.configuration", "settings", &v.position, &v, source).unwrap()
}

fn mapping(entries: Vec<(String, ConfigValue)>) -> ConfigKind {
    ConfigKind::Mapping(
        entries
            .into_iter()
            .map(|(key, value)| ConfigEntry {
                key,
                key_position: value.position.clone(),
                value,
            })
            .collect(),
    )
}

fn number(text: &str) -> ConfigValue {
    value(ConfigKind::Number(text.into()), text)
}

#[test]
fn map_order_is_ignored_but_keys_values_and_sequence_order_are_not() {
    let source = "[1,2]";
    let a = value(ConfigKind::Number("1".into()), source);
    let b = value(ConfigKind::Number("2".into()), source);
    let map = |entries| node(mapping(entries), source);
    let first = map(vec![("a".into(), a.clone()), ("b".into(), b.clone())]);
    let reordered = map(vec![("b".into(), b.clone()), ("a".into(), a.clone())]);
    assert_eq!(first.structural_hash, reordered.structural_hash);
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&reordered).unwrap()
    );
    assert_ne!(
        first.structural_hash,
        map(vec![("changed".into(), a.clone()), ("b".into(), b.clone())]).structural_hash
    );
    assert_ne!(
        first.structural_hash,
        map(vec![("a".into(), b.clone()), ("b".into(), b.clone())]).structural_hash
    );
    assert_ne!(
        node(ConfigKind::Sequence(vec![a.clone(), b.clone()]), source).structural_hash,
        node(ConfigKind::Sequence(vec![b, a]), source).structural_hash
    );
}

#[test]
fn runtime_types_empty_containers_and_field_identity_participate() {
    let source = "false";
    assert_ne!(
        node(ConfigKind::Boolean(false), source).structural_hash,
        node(ConfigKind::String("false".into()), source).structural_hash
    );
    assert_ne!(
        node(mapping(vec![]), source).structural_hash,
        node(ConfigKind::Sequence(vec![]), source).structural_hash
    );
    assert_ne!(
        node(ConfigKind::Null, source).structural_hash,
        node(ConfigKind::String("null".into()), source).structural_hash
    );
    let v = value(ConfigKind::Boolean(false), source);
    assert_ne!(
        configuration_node("id", "retry", &v.position, &v, source)
            .unwrap()
            .structural_hash,
        configuration_node("id", "timeout", &v.position, &v, source)
            .unwrap()
            .structural_hash
    );
}

#[test]
fn exact_numeric_atoms_never_round_through_f64() {
    for (left, right) in [
        ("9007199254740992", "9007199254740993"),
        ("1.00000000000000000001", "1.00000000000000000002"),
        (
            "99999999999999999999999999999999999999999",
            "99999999999999999999999999999999999999998",
        ),
        ("1e9999", "2e9999"),
    ] {
        let a =
            configuration_node("id", "value", &number(left).position, &number(left), left).unwrap();
        let b = configuration_node(
            "id",
            "value",
            &number(right).position,
            &number(right),
            right,
        )
        .unwrap();
        assert_ne!(a.structural_hash, b.structural_hash, "{left} vs {right}");
        assert_eq!(a.children[0].label, left);
        assert_eq!(b.children[0].label, right);
    }
}

#[test]
fn full_unicode_and_long_tail_payloads_and_exact_byte_spans_survive() {
    let left = format!("{}x", "λ😀".repeat(4200));
    let right = format!("{}y", "λ😀".repeat(4200));
    let old_source = serde_json::to_string(&left).unwrap();
    let new_source = serde_json::to_string(&right).unwrap();
    let a = node(ConfigKind::String(left.clone()), &old_source);
    let b = node(ConfigKind::String(right), &new_source);
    assert_ne!(a.structural_hash, b.structural_hash);
    assert_eq!(a.children[0].label, left);
    assert_eq!(a.children[0].position.end_col as usize, old_source.len());
    assert_eq!(a.children[0].position.start_col, 0);
    assert!(a.children[0].facts.is_none());
}

#[test]
fn opaque_fallback_retains_complete_source_without_claiming_equivalence() {
    let left = format!("{} a", "x".repeat(5000));
    let right = format!("{} b", "x".repeat(5000));
    let a = node(ConfigKind::Opaque(left.clone()), &left);
    let b = node(ConfigKind::Opaque(right.clone()), &right);
    assert_eq!(a.children[0].label, left);
    assert_ne!(a.structural_hash, b.structural_hash);
}

#[test]
fn arbitrary_keys_have_unique_deterministic_node_ids() {
    let source = "0";
    let tree = node(
        mapping(vec![
            ("a.b".into(), number("0")),
            (
                "a".into(),
                value(mapping(vec![("b".into(), number("0"))]), source),
            ),
            ("a/0:1\0λ".into(), number("0")),
        ]),
        source,
    );
    fn audit(n: &SemanticNode, ids: &mut HashSet<String>) {
        assert!(ids.insert(n.id.clone()));
        assert!(n.id.starts_with("domain.configuration:"));
        for child in &n.children {
            audit(child, ids);
        }
    }
    let mut ids = HashSet::new();
    audit(&tree, &mut ids);
    assert_eq!(ids.len(), 10);
}

#[test]
fn duplicate_keys_bad_prefix_empty_number_and_invalid_source_ranges_decline() {
    let duplicate = value(
        mapping(vec![("a".into(), number("0")), ("a".into(), number("0"))]),
        "0",
    );
    assert_eq!(
        configuration_node("id", "field", &duplicate.position, &duplicate, "0").unwrap_err(),
        ConfigProjectionError::DuplicateKey
    );
    assert_eq!(
        configuration_node("", "field", &number("0").position, &number("0"), "0").unwrap_err(),
        ConfigProjectionError::EmptyIdPrefix
    );
    assert_eq!(
        configuration_node("id", "field", &number("").position, &number(""), "").unwrap_err(),
        ConfigProjectionError::EmptyNumber
    );
    for p in [
        Position {
            start_line: 0,
            start_col: 1,
            end_line: 0,
            end_col: 2,
        },
        Position {
            start_line: 0,
            start_col: 2,
            end_line: 0,
            end_col: 1,
        },
        Position {
            start_line: 1,
            start_col: 0,
            end_line: 1,
            end_col: 0,
        },
        Position {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 3,
        },
    ] {
        let v = ConfigValue {
            position: p,
            kind: ConfigKind::String("λ".into()),
        };
        assert_eq!(
            configuration_node("id", "field", &v.position, &v, "λ").unwrap_err(),
            ConfigProjectionError::InvalidPosition
        );
    }
}

#[test]
fn recursion_is_bounded_and_source_bounds_do_not_imply_parent_containment() {
    let mut v = number("0");
    for _ in 0..129 {
        v = value(ConfigKind::Sequence(vec![v]), "0");
    }
    assert_eq!(
        configuration_node("id", "field", &v.position, &v, "0").unwrap_err(),
        ConfigProjectionError::DepthLimit
    );
    let mut parent = value(ConfigKind::Sequence(vec![number("0")]), "0");
    parent.position.end_col = 0;
    assert!(configuration_node("id", "field", &parent.position, &parent, "0").is_ok());
}

#[test]
fn reusable_source_index_handles_crlf_eof_and_independent_property_ids() {
    let source = "settings:\r\n  λ: 1\r\n";
    let projector = ConfigurationProjector::new(source);
    let first = ConfigValue {
        position: Position {
            start_line: 1,
            start_col: 6,
            end_line: 1,
            end_col: 7,
        },
        kind: ConfigKind::Number("1".into()),
    };
    let a = projector
        .configuration_node("first", "retry", &first.position, &first)
        .unwrap();
    let b = projector
        .configuration_node("second", "timeout", &first.position, &first)
        .unwrap();
    assert_ne!(a.id, b.id);
    assert_eq!(a.children[0].position.start_col, 6);
    let eof = ConfigValue {
        position: Position {
            start_line: 2,
            start_col: 0,
            end_line: 2,
            end_col: 0,
        },
        kind: ConfigKind::Null,
    };
    assert!(projector
        .configuration_node("eof", "empty", &eof.position, &eof)
        .is_ok());
}

#[test]
fn total_emitted_nodes_are_bounded_including_field_wrappers() {
    let values = vec![number("0"); 100_000];
    let v = value(ConfigKind::Sequence(values), "0");
    assert_eq!(
        configuration_node("id", "field", &v.position, &v, "0").unwrap_err(),
        ConfigProjectionError::NodeLimit
    );
}

#[test]
fn root_key_rename_anchors_key_bytes_and_keeps_exact_scalar_span() {
    for (old_key, new_key) in [
        ("a", "b"),
        ("λ", "μ"),
        ("a\"b", "a\"c"),
        ("", "x"),
        ("a\\b", "a\\c"),
    ] {
        let render = |key: &str| {
            let encoded = serde_json::to_string(key).unwrap();
            let source = format!("{{{encoded}:1}}");
            let scalar = ConfigValue {
                position: Position {
                    start_line: 0,
                    start_col: encoded.len() as u32 + 2,
                    end_line: 0,
                    end_col: encoded.len() as u32 + 3,
                },
                kind: ConfigKind::Number("1".into()),
            };
            let key_position = Position {
                start_line: 0,
                start_col: 1,
                end_line: 0,
                end_col: 1 + encoded.len() as u32,
            };
            let tree = configuration_node("key", key, &key_position, &scalar, &source).unwrap();
            assert_eq!(tree.position.start_col, 1);
            assert_eq!(tree.position.end_col as usize, 1 + encoded.len());
            assert_eq!(
                &source[tree.position.start_col as usize..tree.position.end_col as usize],
                encoded
            );
            assert_eq!(
                &source[tree.children[0].position.start_col as usize
                    ..tree.children[0].position.end_col as usize],
                "1"
            );
            tree
        };
        assert_ne!(
            render(old_key).structural_hash,
            render(new_key).structural_hash
        );
    }
}

#[test]
fn nested_mapping_key_anchors_are_separate_from_unchanged_values() {
    for key in ["a", "λ", "a\"b", "", "a\\b"] {
        let encoded = serde_json::to_string(key).unwrap();
        let source = format!("{{{encoded}:1}}");
        let scalar = ConfigValue {
            position: Position {
                start_line: 0,
                start_col: encoded.len() as u32 + 2,
                end_line: 0,
                end_col: encoded.len() as u32 + 3,
            },
            kind: ConfigKind::Number("1".into()),
        };
        let key_position = Position {
            start_line: 0,
            start_col: 1,
            end_line: 0,
            end_col: 1 + encoded.len() as u32,
        };
        let tree = node(
            ConfigKind::Mapping(vec![ConfigEntry {
                key: key.into(),
                key_position,
                value: scalar,
            }]),
            &source,
        );
        let property = &tree.children[0].children[0];
        assert_eq!(property.position.start_col, 1);
        assert_eq!(property.position.end_col as usize, 1 + encoded.len());
        assert_eq!(
            &source[property.children[0].position.start_col as usize
                ..property.children[0].position.end_col as usize],
            "1"
        );
    }
}

#[test]
fn invalid_key_spans_decline_independently_of_valid_scalar_spans() {
    let source = "{\"λ\":1}";
    let scalar = ConfigValue {
        position: Position {
            start_line: 0,
            start_col: 6,
            end_line: 0,
            end_col: 7,
        },
        kind: ConfigKind::Number("1".into()),
    };
    for bad_key in [
        Position {
            start_line: 0,
            start_col: 3,
            end_line: 0,
            end_col: 5,
        },
        Position {
            start_line: 0,
            start_col: 5,
            end_line: 0,
            end_col: 1,
        },
        Position {
            start_line: 0,
            start_col: 1,
            end_line: 0,
            end_col: 99,
        },
        Position {
            start_line: 1,
            start_col: 0,
            end_line: 1,
            end_col: 0,
        },
    ] {
        assert_eq!(
            configuration_node("id", "λ", &bad_key, &scalar, source).unwrap_err(),
            ConfigProjectionError::InvalidPosition
        );
        let object = value(
            ConfigKind::Mapping(vec![ConfigEntry {
                key: "λ".into(),
                key_position: bad_key,
                value: scalar.clone(),
            }]),
            source,
        );
        assert_eq!(
            configuration_node("id", "context", &object.position, &object, source).unwrap_err(),
            ConfigProjectionError::InvalidPosition
        );
    }
}
