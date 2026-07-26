//! Helpers for parser-owned language metadata bundled into Wasm plugins.
//!
//! Parser crates include a sibling `plugin_metadata.info` file with
//! `include_str!` and expose it through the `language-info` WIT export. The
//! host treats these fields as display/API metadata only; trust, provenance,
//! priority, and plugin identifiers are attached by the host.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginMetadata {
    author: String,
    plugin_version: String,
    last_updated: String,
    languages: BTreeMap<String, LanguageMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageMetadata {
    pub language_id: String,
    pub language_name: String,
    pub language_short_name: String,
    pub monaco_language: String,
    pub default_filename: String,
    pub language_file_extensions: Vec<String>,
}

pub fn parse_plugin_metadata(input: &str) -> PluginMetadata {
    PluginMetadata::parse(input)
}

impl PluginMetadata {
    pub fn parse(input: &str) -> Self {
        let mut metadata = Self {
            author: String::new(),
            plugin_version: String::new(),
            last_updated: String::new(),
            languages: BTreeMap::new(),
        };
        let mut section = Section::None;

        for raw_line in input.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                section = if name == "plugin" {
                    Section::Plugin
                } else if let Some(language_id) = name.strip_prefix("language.") {
                    let language_id = clean_value(language_id);
                    metadata
                        .languages
                        .entry(language_id.clone())
                        .or_insert_with(|| LanguageMetadata::default_for(&language_id));
                    Section::Language(language_id)
                } else {
                    Section::None
                };
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = clean_value(value);
            match &section {
                Section::Plugin => match key {
                    "author" => metadata.author = value,
                    "plugin_version" => metadata.plugin_version = value,
                    "last_updated" => metadata.last_updated = value,
                    _ => {}
                },
                Section::Language(language_id) => {
                    if let Some(language) = metadata.languages.get_mut(language_id) {
                        match key {
                            "language_name" => language.language_name = value,
                            "language_short_name" => language.language_short_name = value,
                            "monaco_language" => language.monaco_language = value,
                            "default_filename" => language.default_filename = value,
                            "language_file_extensions" => {
                                language.language_file_extensions = value
                                    .split(',')
                                    .map(str::trim)
                                    .filter(|item| !item.is_empty())
                                    .map(ToString::to_string)
                                    .collect();
                            }
                            _ => {}
                        }
                    }
                }
                Section::None => {}
            }
        }

        metadata
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn plugin_version(&self) -> &str {
        &self.plugin_version
    }

    pub fn last_updated(&self) -> &str {
        &self.last_updated
    }

    pub fn language_or_default(&self, language_id: &str) -> LanguageMetadata {
        self.languages
            .get(language_id)
            .cloned()
            .unwrap_or_else(|| LanguageMetadata::default_for(language_id))
    }
}

impl LanguageMetadata {
    fn default_for(language_id: &str) -> Self {
        let name = default_language_name(language_id);
        Self {
            language_id: language_id.to_string(),
            language_name: name.clone(),
            language_short_name: name,
            monaco_language: "plaintext".to_string(),
            default_filename: format!("code.{language_id}"),
            language_file_extensions: Vec::new(),
        }
    }
}

enum Section {
    None,
    Plugin,
    Language(String),
}

fn clean_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let first = trimmed.as_bytes()[0];
        let last = trimmed.as_bytes()[trimmed.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn default_language_name(language_id: &str) -> String {
    language_id
        .replace('_', "-")
        .split('-')
        .map(|part| {
            if part.len() <= 3 {
                part.to_uppercase()
            } else {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plugin_and_language_sections() {
        let metadata = parse_plugin_metadata(
            r#"
            [plugin]
            author = Example Author
            plugin_version = 1.2.3
            last_updated = 2026-05-19

            [language.python]
            language_name = Python
            language_short_name = Py
            monaco_language = python
            default_filename = code.py
            language_file_extensions = .py, .pyi
            "#,
        );

        let python = metadata.language_or_default("python");

        assert_eq!(metadata.author(), "Example Author");
        assert_eq!(metadata.plugin_version(), "1.2.3");
        assert_eq!(metadata.last_updated(), "2026-05-19");
        assert_eq!(python.language_name, "Python");
        assert_eq!(python.language_short_name, "Py");
        assert_eq!(python.monaco_language, "python");
        assert_eq!(python.default_filename, "code.py");
        assert_eq!(python.language_file_extensions, vec![".py", ".pyi"]);
    }

    #[test]
    fn missing_language_uses_display_safe_defaults() {
        let metadata = parse_plugin_metadata("");
        let info = metadata.language_or_default("dbt-jinja");

        assert_eq!(info.language_name, "DBT Jinja");
        assert_eq!(info.monaco_language, "plaintext");
        assert_eq!(info.default_filename, "code.dbt-jinja");
    }
}
