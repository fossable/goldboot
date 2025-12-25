use anyhow::Result;
use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use crate::builder::os::Os;

mod starlark;
pub mod starlark_dsl;

/// Get the Goldboot Starlark DSL for LSP support
pub fn get_starlark_dsl() -> String {
    starlark_dsl::get_dsl()
}

/// Represents a builder configuration file path
#[derive(Clone, Debug)]
pub struct ConfigPath(PathBuf);

impl Default for ConfigPath {
    fn default() -> Self {
        ConfigPath(PathBuf::from("./goldboot.star"))
    }
}

impl ConfigPath {
    /// Check for a builder configuration file in the given directory.
    pub fn from_dir(path: impl AsRef<Path>) -> Option<ConfigPath> {
        let path = path.as_ref();

        if path.join("goldboot.star").exists() {
            return Some(ConfigPath(path.join("goldboot.star")));
        }

        None
    }

    /// Read and evaluate the Starlark configuration file.
    pub fn load(&self) -> Result<Vec<Os>> {
        starlark::load_config(&self.0)
    }

    /// Write a new Starlark configuration file.
    pub fn write(&self, elements: &Vec<Os>) -> Result<()> {
        let starlark_code = to_starlark_code(elements)?;
        std::fs::write(&self.0, starlark_code.as_bytes())?;
        Ok(())
    }
}

impl Display for ConfigPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.to_string_lossy().fmt(f)
    }
}

/// Convert Os elements to Starlark code
fn to_starlark_code(elements: &[Os]) -> Result<String> {
    if elements.len() == 1 {
        format_os(&elements[0])
    } else {
        let items: Result<Vec<String>> = elements.iter().map(|os| format_os(os)).collect();
        Ok(format!("[\n{}\n]\n", items?.join(",\n")))
    }
}

fn format_os(os: &Os) -> Result<String> {
    // Serialize to JSON first, then convert to Starlark syntax
    let json = serde_json::to_value(os)?;
    Ok(json_to_starlark(&json, 0))
}

fn json_to_starlark(value: &serde_json::Value, indent: usize) -> String {
    json_to_starlark_with_context(value, indent, None)
}

fn json_to_starlark_with_context(
    value: &serde_json::Value,
    indent: usize,
    field_name: Option<&str>,
) -> String {
    match value {
        serde_json::Value::Null => "None".to_string(),
        serde_json::Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            format!("\"{}\"", s.replace("\\", "\\\\").replace("\"", "\\\""))
        }
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                "[]".to_string()
            } else {
                let items: Vec<String> = arr
                    .iter()
                    .map(|v| json_to_starlark_with_context(v, indent + 1, None))
                    .collect();
                format!("[{}]", items.join(", "))
            }
        }
        serde_json::Value::Object(obj) => {
            // Check if this is an Os variant (has "os" tag)
            if let Some(os_type) = obj.get("os").and_then(|v| v.as_str()) {
                // This is an OS constructor
                let indent_str = "    ".repeat(indent + 1);
                let close_indent_str = "    ".repeat(indent);
                let mut args = Vec::new();

                for (key, val) in obj.iter() {
                    if key != "os" {
                        // Skip None values and arch field (has default)
                        if !val.is_null() && key != "arch" {
                            args.push(format!(
                                "{}{}={}",
                                indent_str,
                                key,
                                json_to_starlark_with_context(val, indent + 1, Some(key))
                            ));
                        }
                    }
                }

                if args.is_empty() {
                    format!("{}()", os_type)
                } else {
                    format!("{}(\n{},\n{})", os_type, args.join(",\n"), close_indent_str)
                }
            } else if let Some(field_name) = field_name {
                // Check if this is an enum variant (has single key)
                if obj.len() == 1 {
                    let (key, val) = obj.iter().next().unwrap();
                    // This looks like an enum variant like {"plaintext": "password"}
                    let variant_name = snake_to_upper_camel(key);
                    match val {
                        serde_json::Value::String(s) => {
                            // Simple enum variant with string value
                            return format!("{}(\"{}\")", variant_name, s.replace("\\", "\\\\").replace("\"", "\\\""));
                        }
                        _ => {
                            // Complex enum variant
                            return format!("{}({})", variant_name, json_to_starlark_with_context(val, indent, None));
                        }
                    }
                }

                // This is a nested struct - use the field name to infer the type
                let type_name = snake_to_upper_camel(field_name);
                let indent_str = "    ".repeat(indent + 1);
                let close_indent_str = "    ".repeat(indent);
                let mut args = Vec::new();

                for (key, val) in obj.iter() {
                    // Skip None/null values
                    if !val.is_null() {
                        args.push(format!(
                            "{}{}={}",
                            indent_str,
                            key,
                            json_to_starlark_with_context(val, indent + 1, Some(key))
                        ));
                    }
                }

                if args.is_empty() {
                    format!("{}()", type_name)
                } else {
                    format!(
                        "{}(\n{},\n{})",
                        type_name,
                        args.join(",\n"),
                        close_indent_str
                    )
                }
            } else {
                // Regular dict (no field context)
                let items: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "\"{}\": {}",
                            k,
                            json_to_starlark_with_context(v, indent + 1, None)
                        )
                    })
                    .collect();
                format!("{{{}}}", items.join(", "))
            }
        }
    }
}

fn snake_to_upper_camel(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}
