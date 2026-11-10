// Compile the actual rendered template with the documented Cargo dependency name.
include!(concat!(env!("OUT_DIR"), "/plugin.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    intentumdiff_plugin_sdk::plugin_compliance_tests! {
        process: process_impl,
        detect_fn: detect_language,
        detect_cases: [("file.fixture", "", "fixture"), ("file.txt", "", "")],
        grammar_id: GRAMMAR_ID,
        language_ids: ["fixture"],
        samples: ["statement\nother statement\n"],
        exported_grammar_id: grammar_id,
        exported_language_ids: language_ids,
    }

    #[test]
    fn rendered_template_parses_source() {
        let output: serde_json::Value =
            serde_json::from_str(&process("statement", "fixture", "file.fixture")).unwrap();
        assert_eq!(output["node_type"], "source_file");
        assert_eq!(output["children"][0]["label"], "statement");
    }
}
