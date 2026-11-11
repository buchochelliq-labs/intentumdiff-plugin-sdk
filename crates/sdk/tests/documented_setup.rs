//! Issue #27: the documented dependency, the distributed template and the SDK must agree,
//! so a plugin generated outside this workspace resolves and builds.
//!
//! The rendered template itself is compiled and tested by `tests/authoring-standard`.

use std::path::PathBuf;

fn repo_file(path: &str) -> String {
    let full = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("read {}: {e}", full.display()))
}

/// The tag every documented git dependency must name: the SDK's own version, so the
/// pinned tag carries this package name (`v0.1.0` still names `intentdiff-plugin-sdk`).
fn expected_tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn sdk_git_tags(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| {
            line.contains("intentumdiff-plugin-sdk")
                && line.contains("git = \"https://github.com/buchochelliq-labs/intentumdiff-plugin-sdk\"")
        })
        .map(|line| {
            let tag = line
                .split("tag = \"")
                .nth(1)
                .unwrap_or_else(|| panic!("SDK git dependency without a tag: {line}"));
            tag[..tag.find('"').unwrap()].to_owned()
        })
        .collect()
}

/// Published tags whose package is still the pre-rebrand `intentdiff-plugin-sdk`: a
/// consumer declaring `intentumdiff-plugin-sdk` at these tags fails with "no matching
/// package named `intentumdiff-plugin-sdk`".
const PRE_REBRAND_TAGS: &[&str] = &["v0.1.0"];

#[test]
fn documented_sdk_tags_match_the_sdk_version() {
    assert!(
        !PRE_REBRAND_TAGS.contains(&expected_tag().as_str()),
        "SDK version {} collides with a pre-rebrand tag",
        env!("CARGO_PKG_VERSION")
    );
    for path in [
        "templates/plugin-template/Cargo.toml.liquid",
        "README.md",
        "docs/BUILDING.md",
        "crates/sdk/src/lib.rs",
    ] {
        let tags = sdk_git_tags(&repo_file(path));
        assert!(!tags.is_empty(), "{path} documents no SDK git dependency");
        for tag in tags {
            assert_eq!(tag, expected_tag(), "{path} pins SDK tag {tag}");
        }
    }
}

#[test]
fn template_declares_runtime_and_testing_dependencies() {
    let manifest = repo_file("templates/plugin-template/Cargo.toml.liquid");
    let dev = manifest
        .split("[dev-dependencies]")
        .nth(1)
        .expect("template has [dev-dependencies]");
    assert!(
        dev.contains("intentumdiff-plugin-sdk") && dev.contains("features = [\"testing\"]"),
        "template dev-dependency must enable the SDK `testing` feature"
    );
    assert!(manifest.contains("crate-type = [\"cdylib\"]"));
}

#[test]
fn template_ships_the_master_wit_contract() {
    assert_eq!(
        repo_file("templates/plugin-template/wit/plugin.wit"),
        repo_file("wit/plugin.wit"),
        "templates/plugin-template/wit/plugin.wit must be a byte-identical copy of wit/plugin.wit"
    );
    let source = repo_file("templates/plugin-template/src/lib.rs");
    assert!(source.contains("wit_bindgen::generate!"));
    assert!(source.contains("export!("));
    assert!(source.contains("plugin_compliance_tests!"));
}
