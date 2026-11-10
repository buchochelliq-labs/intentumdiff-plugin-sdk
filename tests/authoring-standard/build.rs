use std::{env, fs, path::PathBuf};

// Render the ACTUAL distributed template (src/lib.rs + plugin_metadata.info) the way
// cargo-generate does for the two placeholders, so this crate compiles and tests exactly
// what a plugin author receives — WIT exports, metadata and compliance suite included.
fn render(text: &str) -> String {
    text.replace("{{project-name}}", "sdk-authoring-standard")
        .replace("{{language}}", "fixture")
}

fn main() {
    let template = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../templates/plugin-template")
        .canonicalize()
        .expect("locate the plugin template");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for file in ["src/lib.rs", "plugin_metadata.info", "wit/plugin.wit", "Cargo.toml.liquid"] {
        println!("cargo:rerun-if-changed={}", template.join(file).display());
    }

    let rendered = render(&fs::read_to_string(template.join("src/lib.rs")).unwrap());
    assert!(!rendered.contains("{{") && !rendered.contains("}}"), "unrendered Liquid in lib.rs");
    let metadata = out.join("plugin_metadata.info");
    fs::write(&metadata, render(&fs::read_to_string(template.join("plugin_metadata.info")).unwrap()))
        .unwrap();
    let wit = template.join("wit/plugin.wit");
    let source = rendered
        // Included source cannot contain crate-level inner doc comments.
        .replace("//!", "//")
        // Crate-relative paths resolve against the generated crate's root.
        .replace("path: \"wit/plugin.wit\"", &format!("path: {:?}", wit.display().to_string()))
        .replace(
            "include_str!(\"../plugin_metadata.info\")",
            &format!("include_str!({:?})", metadata.display().to_string()),
        );
    fs::write(out.join("plugin.rs"), source).expect("render the actual plugin template");
}
