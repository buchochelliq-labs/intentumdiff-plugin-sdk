#[cfg(test)]
mod tests {
    fn process(_source: &str) -> String {
        "{}".to_owned()
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
