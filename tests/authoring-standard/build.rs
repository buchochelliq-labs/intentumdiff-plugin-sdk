use std::{env, fs, path::PathBuf};

fn main() {
    let template = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../templates/plugin-template/src/lib.rs");
    println!("cargo:rerun-if-changed={}", template.display());
    let source = fs::read_to_string(template)
        .expect("read the actual plugin template")
        .replace("{{project-name}}", "sdk-authoring-standard")
        .replace("{{language}}", "fixture")
        // Included source cannot contain crate-level inner doc comments.
        .replace("//!", "//");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("plugin.rs");
    fs::write(output, source).expect("render the actual plugin template");
}
