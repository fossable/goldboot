use anyhow::Result;
use clap::{ValueEnum, builder::PossibleValue};
#[cfg(feature = "config-python")]
use pyo3::{
    ffi::c_str,
    prelude::*,
    types::{IntoPyDict, PyModule},
};
use serde::Deserialize;
use std::{
    ffi::CString,
    fmt::Display,
    path::{Path, PathBuf},
    sync::OnceLock,
    thread,
    time::SystemTime,
};
use strum::EnumIter;

use crate::builder::{Builder, os::Os};

#[cfg(feature = "config-python")]
mod python_codegen {
    use super::Os;

    /// Convert Os elements to Python code
    pub fn to_python_code(elements: &[Os]) -> Result<String, serde_json::Error> {
        if elements.len() == 1 {
            format_os(&elements[0])
        } else {
            let items: Result<Vec<String>, _> = elements.iter().map(|os| format_os(os)).collect();
            Ok(format!("[\n{}\n]\n", items?.join(",\n")))
        }
    }

    fn format_os(os: &Os) -> Result<String, serde_json::Error> {
        // Serialize to JSON first, then convert to Python syntax
        let json = serde_json::to_value(os)?;
        Ok(json_to_python(&json, 0))
    }

    fn json_to_python(value: &serde_json::Value, indent: usize) -> String {
        json_to_python_with_context(value, indent, None)
    }

    fn json_to_python_with_context(
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
                        .map(|v| json_to_python_with_context(v, indent + 1, None))
                        .collect();
                    format!("[{}]", items.join(", "))
                }
            }
            serde_json::Value::Object(obj) => {
                // Check if this is an Os variant (has "os" tag)
                if let Some(os_type) = obj.get("os").and_then(|v| v.as_str()) {
                    // This is an Os constructor
                    let indent_str = "    ".repeat(indent + 1);
                    let close_indent_str = "    ".repeat(indent);
                    let mut args = Vec::new();

                    for (key, val) in obj.iter() {
                        if key != "os" {
                            args.push(format!(
                                "{}{}={}",
                                indent_str,
                                key,
                                json_to_python_with_context(val, indent + 1, Some(key))
                            ));
                        }
                    }

                    if args.is_empty() {
                        format!("{}()", os_type)
                    } else {
                        format!("{}(\n{},\n{})", os_type, args.join(",\n"), close_indent_str)
                    }
                } else if let Some(field_name) = field_name {
                    // This is a nested struct - use the field name to infer the type
                    let type_name = snake_to_upper_camel(field_name);
                    let indent_str = "    ".repeat(indent + 1);
                    let close_indent_str = "    ".repeat(indent);
                    let mut args = Vec::new();

                    for (key, val) in obj.iter() {
                        args.push(format!(
                            "{}{}={}",
                            indent_str,
                            key,
                            json_to_python_with_context(val, indent + 1, Some(key))
                        ));
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
                                json_to_python_with_context(v, indent + 1, None)
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_json_to_python_primitives() {
            use serde_json::json;

            assert_eq!(json_to_python(&json!(null), 0), "None");
            assert_eq!(json_to_python(&json!(true), 0), "True");
            assert_eq!(json_to_python(&json!(false), 0), "False");
            assert_eq!(json_to_python(&json!(42), 0), "42");
            assert_eq!(json_to_python(&json!(3.14), 0), "3.14");
            assert_eq!(json_to_python(&json!("hello"), 0), "\"hello\"");
        }

        #[test]
        fn test_json_to_python_string_escaping() {
            use serde_json::json;

            assert_eq!(
                json_to_python(&json!("hello \"world\""), 0),
                "\"hello \\\"world\\\"\""
            );
            assert_eq!(
                json_to_python(&json!("path\\to\\file"), 0),
                "\"path\\\\to\\\\file\""
            );
        }

        #[test]
        fn test_json_to_python_array() {
            use serde_json::json;

            assert_eq!(json_to_python(&json!([]), 0), "[]");
            assert_eq!(json_to_python(&json!([1, 2, 3]), 0), "[1, 2, 3]");
            assert_eq!(
                json_to_python(&json!(["a", "b", "c"]), 0),
                "[\"a\", \"b\", \"c\"]"
            );
        }

        #[test]
        fn test_json_to_python_dict() {
            use serde_json::json;

            let result = json_to_python(&json!({"key": "value"}), 0);
            assert_eq!(result, "{\"key\": \"value\"}");
        }

        #[test]
        fn test_json_to_python_os_variant() {
            use serde_json::json;

            let os_json = json!({
                "os": "ArchLinux",
                "hostname": "test",
                "mirrorlist": []
            });

            let result = json_to_python(&os_json, 0);
            assert!(result.starts_with("ArchLinux("));
            assert!(result.contains("hostname=\"test\""));
            assert!(result.contains("mirrorlist=[]"));
        }

        #[test]
        fn test_json_to_python_nested_object() {
            use serde_json::json;

            let os_json = json!({
                "os": "ArchLinux",
                "source": {
                    "url": "http://example.com",
                    "checksum": "abc123"
                }
            });

            let result = json_to_python(&os_json, 0);
            assert!(result.starts_with("ArchLinux("));
            assert!(result.contains("source=Source("));
            assert!(result.contains("url=\"http://example.com\""));
            assert!(result.contains("checksum=\"abc123\""));
        }

        #[test]
        fn test_to_python_code_single() {
            use crate::builder::os::{Os, arch_linux::ArchLinux};

            let elements = vec![Os::ArchLinux(ArchLinux::default())];
            let result = to_python_code(&elements).unwrap();

            assert!(result.starts_with("ArchLinux("));
            assert!(!result.starts_with("["));
        }

        #[test]
        fn test_to_python_code_multiple() {
            use crate::builder::os::{Os, arch_linux::ArchLinux};

            let elements = vec![
                Os::ArchLinux(ArchLinux::default()),
                Os::ArchLinux(ArchLinux::default()),
            ];
            let result = to_python_code(&elements).unwrap();

            assert!(result.starts_with("["));
            assert!(result.ends_with("]\n"));
        }
    }
}

/// Represents a builder configuration file. This mainly helps sort out the various
/// supported config formats.
#[derive(Clone, Debug, EnumIter)]
pub enum ConfigPath {
    #[cfg(feature = "config-json")]
    Json(PathBuf),
    #[cfg(feature = "config-python")]
    Python(PathBuf),
    #[cfg(feature = "config-ron")]
    Ron(PathBuf),
    #[cfg(feature = "config-toml")]
    Toml(PathBuf),
    #[cfg(feature = "config-yaml")]
    Yaml(PathBuf),
}

impl Default for ConfigPath {
    fn default() -> Self {
        ConfigPath::Python(PathBuf::from("./goldboot.py"))
    }
}

static VARIANTS: OnceLock<Vec<ConfigPath>> = OnceLock::new();

impl ValueEnum for ConfigPath {
    fn value_variants<'a>() -> &'a [Self] {
        VARIANTS.get_or_init(|| {
            vec![
                #[cfg(feature = "config-json")]
                ConfigPath::Json(PathBuf::from("./goldboot.json")),
                #[cfg(feature = "config-python")]
                ConfigPath::Python(PathBuf::from("./goldboot.py")),
                #[cfg(feature = "config-ron")]
                ConfigPath::Ron(PathBuf::from("./goldboot.ron")),
                #[cfg(feature = "config-toml")]
                ConfigPath::Toml(PathBuf::from("./goldboot.toml")),
                #[cfg(feature = "config-yaml")]
                ConfigPath::Yaml(PathBuf::from("./goldboot.yaml")),
            ]
        })
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match *self {
            #[cfg(feature = "config-json")]
            ConfigPath::Json(_) => Some(PossibleValue::new("json")),
            #[cfg(feature = "config-python")]
            ConfigPath::Python(_) => Some(PossibleValue::new("python")),
            #[cfg(feature = "config-ron")]
            ConfigPath::Ron(_) => Some(PossibleValue::new("ron")),
            #[cfg(feature = "config-toml")]
            ConfigPath::Toml(_) => Some(PossibleValue::new("toml")),
            #[cfg(feature = "config-yaml")]
            ConfigPath::Yaml(_) => Some(PossibleValue::new("yaml")),
        }
    }
}

/// Helper enum to deserialize either a single Os or a Vec<Os>
#[derive(Deserialize)]
#[serde(untagged)]
enum SingleOrMultiple {
    Single(Os),
    Multiple(Vec<Os>),
}

impl From<SingleOrMultiple> for Vec<Os> {
    fn from(value: SingleOrMultiple) -> Self {
        match value {
            SingleOrMultiple::Single(os) => vec![os],
            SingleOrMultiple::Multiple(vec) => vec,
        }
    }
}

impl ConfigPath {
    /// Check for a builder configuration file in the given directory.
    pub fn from_dir(path: impl AsRef<Path>) -> Option<ConfigPath> {
        let path = path.as_ref();

        #[cfg(feature = "config-python")]
        if path.join("goldboot.py").exists() {
            return Some(ConfigPath::Python(path.join("goldboot.py")));
        }

        #[cfg(feature = "config-json")]
        if path.join("goldboot.json").exists() {
            return Some(ConfigPath::Json(path.join("goldboot.json")));
        }

        #[cfg(feature = "config-ron")]
        if path.join("goldboot.ron").exists() {
            return Some(ConfigPath::Ron(path.join("goldboot.ron")));
        }

        #[cfg(feature = "config-toml")]
        if path.join("goldboot.toml").exists() {
            return Some(ConfigPath::Toml(path.join("goldboot.toml")));
        }

        #[cfg(feature = "config-yaml")]
        if path.join("goldboot.yaml").exists() {
            return Some(ConfigPath::Yaml(path.join("goldboot.yaml")));
        } else if path.join("goldboot.yml").exists() {
            return Some(ConfigPath::Yaml(path.join("goldboot.yml")));
        }

        None
    }

    /// Read or execute the configuration.
    pub fn load(&self) -> Result<Vec<Os>> {
        Ok(match &self {
            #[cfg(feature = "config-json")]
            Self::Json(path) => {
                let os_or_vec: SingleOrMultiple = serde_json::from_slice(&std::fs::read(path)?)?;
                os_or_vec.into()
            }
            #[cfg(feature = "config-python")]
            Self::Python(path) => Python::attach(|py| {
                // Execute the Python file and get the last expression's value
                let code = std::fs::read_to_string(path)?;
                let result = py.eval(CString::new(code)?.as_c_str(), None, None)?;

                // Try to extract as Vec<Os> first (list case)
                if let Ok(os_list) = result.extract::<Vec<Os>>() {
                    return Ok(os_list);
                }

                // Otherwise, try to extract as single Os and wrap in Vec
                if let Ok(os) = result.extract::<Os>() {
                    return Ok(vec![os]);
                }

                Err(anyhow::anyhow!(
                    "Failed to extract Os configuration from Python module"
                ))
            })?,
            #[cfg(feature = "config-ron")]
            Self::Ron(path) => {
                let os_or_vec: SingleOrMultiple = ron::de::from_bytes(&std::fs::read(path)?)?;
                os_or_vec.into()
            }
            #[cfg(feature = "config-toml")]
            Self::Toml(path) => {
                let os_or_vec: SingleOrMultiple =
                    toml::from_str(String::from_utf8(std::fs::read(path)?)?.as_str())?;
                os_or_vec.into()
            }
            #[cfg(feature = "config-yaml")]
            Self::Yaml(path) => {
                let os_or_vec: SingleOrMultiple = serde_yaml::from_slice(&std::fs::read(path)?)?;
                os_or_vec.into()
            }
        })
    }

    /// Write a new configuration file.
    pub fn write(&self, elements: &Vec<Os>) -> Result<()> {
        match &self {
            #[cfg(feature = "config-json")]
            Self::Json(path) => {
                if elements.len() == 1 {
                    std::fs::write(path, serde_json::to_vec_pretty(&elements[0])?)
                } else {
                    std::fs::write(path, serde_json::to_vec_pretty(elements)?)
                }
            }
            #[cfg(feature = "config-python")]
            Self::Python(path) => {
                let python_code = python_codegen::to_python_code(elements)?;
                std::fs::write(path, python_code.as_bytes())
            }
            #[cfg(feature = "config-ron")]
            Self::Ron(path) => {
                let content = if elements.len() == 1 {
                    ron::ser::to_string_pretty(&elements[0], ron::ser::PrettyConfig::new())?
                } else {
                    ron::ser::to_string_pretty(elements, ron::ser::PrettyConfig::new())?
                };
                std::fs::write(path, content.into_bytes())
            }
            #[cfg(feature = "config-toml")]
            Self::Toml(path) => {
                let content = if elements.len() == 1 {
                    toml::to_string_pretty(&elements[0])?
                } else {
                    toml::to_string_pretty(elements)?
                };
                std::fs::write(path, content.into_bytes())
            }
            #[cfg(feature = "config-yaml")]
            Self::Yaml(path) => {
                let content = if elements.len() == 1 {
                    serde_yaml::to_string(&elements[0])?
                } else {
                    serde_yaml::to_string(elements)?
                };
                std::fs::write(path, content.into_bytes())
            }
        }?;
        Ok(())
    }
}

impl Display for ConfigPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = match self {
            #[cfg(feature = "config-json")]
            ConfigPath::Json(path) => path,
            #[cfg(feature = "config-python")]
            ConfigPath::Python(path) => path,
            #[cfg(feature = "config-ron")]
            ConfigPath::Ron(path) => path,
            #[cfg(feature = "config-toml")]
            ConfigPath::Toml(path) => path,
            #[cfg(feature = "config-yaml")]
            ConfigPath::Yaml(path) => path,
        }
        .to_string_lossy();
        path.fmt(f)
    }
}
