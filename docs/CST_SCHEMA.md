# CST JSON schema (interpret-cst compatibility)

The legacy JSON format `interpret-cst` mode plugins can receive in `process()`. Product parser
paths are Rust/Wasm **full-parse** (raw source in); this schema is retained as a
compatibility + test-oracle reference.

Every node is a JSON object; leaves carry text, internal nodes carry children:

```json
{"type": "identifier", "named": true, "text": "calculate_total",
 "start_line": 4, "start_col": 4, "end_line": 4, "end_col": 19}

{"type": "function_definition", "named": true,
 "start_line": 4, "start_col": 0, "end_line": 8, "end_col": 0,
 "children": [ {"type": "def", "named": false, "text": "def"}, ... ]}
```

| Field | Type | On | Meaning |
|---|---|---|---|
| `type` | string | all | grammar node type name |
| `named` | bool | all | named node vs anonymous token |
| `text` | string | leaves + literal containers | complete source text (no truncation); for a kept anonymous token, its grammar token kind |
| `start_line`/`start_col`/`end_line`/`end_col` | int | all | **0-indexed** positions |
| `children` | array | internal | child nodes in source order |

The SDK's `ts_convert::node_to_cst` emits named children only; `node_to_cst_with` can also keep
anonymous operator/keyword tokens as `"named": false` leaves (see the plugin guide).

Full-parse plugins ignore this format entirely — they emit `SemanticNode` trees per the
[plugin guide](PLUGIN_GUIDE.md).
