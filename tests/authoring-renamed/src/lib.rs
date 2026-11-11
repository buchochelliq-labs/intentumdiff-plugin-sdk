#[cfg(test)]
mod tests {
    // A real (if trivial) tree: the compliance suite rejects `{}`, error envelopes and
    // non-tree JSON (issue #24).
    fn process(source: &str) -> String {
        let lines: Vec<&str> = source.split('\n').collect();
        let end_line = (lines.len() - 1) as u32;
        let end_col = lines.last().map_or(0, |line| line.len()) as u32;
        let root = parser_sdk::SemanticNodeBuilder::new(
            "0", "document", "fixture", 0, 0, end_line, end_col, "",
        )
        .build();
        parser_sdk::serde_json::to_string(&root).unwrap()
    }

    fn detect_language(filename: &str, _content: &str) -> String {
        if filename.ends_with(".fixture") {
            "fixture".to_owned()
        } else {
            String::new()
        }
    }

    // This Cargo alias deliberately provides neither the old nor current crate name.
    parser_sdk::plugin_compliance_tests! {
        process: process,
        detect_fn: detect_language,
        detect_cases: [("file.fixture", "", "fixture"), ("file.txt", "", "")],
        grammar_id: "fixture",
        language_ids: ["fixture"],
    }
}
