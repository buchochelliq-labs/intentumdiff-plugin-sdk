// Compile the actual rendered template with the documented Cargo dependency name. Its own
// `compliance` test module (the SDK suite, with samples + exported metadata) runs here.
include!(concat!(env!("OUT_DIR"), "/plugin.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_template_parses_source() {
        let output: serde_json::Value = serde_json::from_str(&Plugin::process(
            "statement".into(),
            "fixture".into(),
            "file.fixture".into(),
        ))
        .unwrap();
        assert_eq!(output["node_type"], "source_file");
        assert_eq!(output["children"][0]["label"], "statement");
    }
}
