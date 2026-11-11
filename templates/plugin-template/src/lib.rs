//! {{project-name}} - IntentumDiff parser plugin.
//!
//! Language: {{language}}
//! Mode:     full-parse (plugin receives raw source and owns parsing)
//!
//! Steps to implement:
//!   1. Compile or implement your parser inside this Rust/Wasm crate.
//!   2. Implement `parse_source()` to emit a SemanticNode tree from raw source.
//!   3. Keep labels, spans, and structural hashes deterministic.
//!   4. Build: `cargo build --release --target wasm32-wasip2`
//!   5. Test:  `cargo test` (runs the SDK compliance suite below)
//!   6. Register through intentumdiff-registry (see the SDK's docs/PLUGIN_GUIDE.md).

use intentumdiff_plugin_sdk::tree::{SemanticNode, SemanticNodeBuilder};

wit_bindgen::generate!({
    path: "wit/plugin.wit",
    world: "parser-plugin",
});

use crate::exports::intentdiff::plugin::parser::{
    ExamplePair, Guest, LanguageInfoRecord, ParserMode,
};

const GRAMMAR_ID: &str = "{{language}}";
const LANGUAGE_IDS: &[&str] = &["{{language}}"];
const EXAMPLE_OLD: &str = "first statement\n";
const EXAMPLE_NEW: &str = "first statement\nsecond statement\n";
const PLUGIN_METADATA: &str = include_str!("../plugin_metadata.info");

fn is_interesting_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with('#')
}

// Positions are 0-based lines and byte columns (the SDK compliance suite rejects spans
// that fall outside the source).
fn node_for_line(index: usize, line: &str) -> Option<SemanticNode> {
    if !is_interesting_line(line) {
        return None;
    }

    let line_no = index as u32;
    let trimmed = line.trim();
    Some(
        SemanticNodeBuilder::new(
            format!("0.{index}"),
            "statement",
            trimmed,
            line_no,
            0,
            line_no,
            line.len() as u32,
            "",
        )
        .build(),
    )
}

fn parse_source(source: &str) -> SemanticNode {
    let lines: Vec<&str> = source.split('\n').collect();
    let children: Vec<SemanticNode> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| node_for_line(index, line))
        .collect();

    let end_line = (lines.len() - 1) as u32;
    let end_col = lines.last().map_or(0, |line| line.len()) as u32;
    SemanticNodeBuilder::new("0", "source_file", GRAMMAR_ID, 0, 0, end_line, end_col, "")
        .children(children)
        .build()
}

fn process_impl(source: &str) -> String {
    // (No literal double braces in this file: cargo-generate renders it as Liquid.)
    serde_json::to_string(&parse_source(source)).unwrap_or_else(|error| {
        serde_json::json!({ "error": format!("serialization failed: {error}") }).to_string()
    })
}

fn detect_language_impl(filename: &str, _content: &str) -> String {
    if filename.ends_with(".{{language}}") {
        GRAMMAR_ID.to_string()
    } else {
        String::new()
    }
}

/// The plugin's WIT exports (`intentdiff:plugin/parser@1.0.0`).
pub struct Plugin;

impl Guest for Plugin {
    fn get_parser_mode() -> ParserMode {
        ParserMode::FullParse
    }

    fn grammar_id() -> String {
        GRAMMAR_ID.to_string()
    }

    fn detect_language(filename: String, content: String) -> String {
        detect_language_impl(&filename, &content)
    }

    fn process(input: String, _language: String, _filename: String) -> String {
        process_impl(&input)
    }

    fn trivia_node_types() -> Vec<String> {
        Vec::new()
    }

    fn language_ids() -> Vec<String> {
        LANGUAGE_IDS.iter().map(|id| id.to_string()).collect()
    }

    fn language_info() -> Vec<LanguageInfoRecord> {
        let metadata = intentumdiff_plugin_sdk::metadata::parse_plugin_metadata(PLUGIN_METADATA);
        Self::language_ids()
            .into_iter()
            .map(|language_id| {
                let info = metadata.language_or_default(&language_id);
                LanguageInfoRecord {
                    language_id: info.language_id,
                    language_name: info.language_name,
                    language_short_name: info.language_short_name,
                    monaco_language: info.monaco_language,
                    default_filename: info.default_filename,
                    language_file_extensions: info.language_file_extensions,
                    author: metadata.author().to_string(),
                    plugin_version: metadata.plugin_version().to_string(),
                    last_updated: metadata.last_updated().to_string(),
                }
            })
            .collect()
    }

    fn priority() -> i32 {
        0
    }

    fn preprocess_source(source: String) -> String {
        source
    }

    fn example(_language: String) -> ExamplePair {
        ExamplePair {
            old: EXAMPLE_OLD.to_string(),
            new: EXAMPLE_NEW.to_string(),
        }
    }
}

export!(Plugin);

#[cfg(test)]
mod compliance {
    use super::*;

    intentumdiff_plugin_sdk::plugin_compliance_tests! {
        process: process_impl,
        detect_fn: detect_language_impl,
        detect_cases: [("file.{{language}}", "", "{{language}}"), ("file.unrelated", "", "")],
        grammar_id: GRAMMAR_ID,
        language_ids: ["{{language}}"],
        samples: [EXAMPLE_OLD, EXAMPLE_NEW],
        exported_grammar_id: Plugin::grammar_id,
        exported_language_ids: Plugin::language_ids,
    }

    #[test]
    fn parser_mode_is_full_parse() {
        assert!(matches!(Plugin::get_parser_mode(), ParserMode::FullParse));
    }

    #[test]
    fn language_info_covers_every_language_id() {
        let info = Plugin::language_info();
        let ids: Vec<String> = info.iter().map(|record| record.language_id.clone()).collect();
        assert_eq!(ids, Plugin::language_ids());
    }
}
