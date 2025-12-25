use anyhow::Result;
use serde::Deserialize;
use starlark::environment::{GlobalsBuilder, Module};
use starlark::eval::Evaluator;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::{dict::DictRef, list::ListRef, Value};
use std::path::Path;

use crate::builder::os::Os;
use super::starlark_dsl;

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

/// Load a Starlark configuration file and return a vector of Os configurations
pub fn load_config(path: impl AsRef<Path>) -> Result<Vec<Os>> {
    let user_content = std::fs::read_to_string(path.as_ref())?;

    // Prepend the DSL to the user's content so all functions are available
    let dsl = starlark_dsl::get_dsl();
    let combined_content = format!("{}\n\n{}", dsl, user_content);

    // Create a module
    let module = Module::new();
    let globals = GlobalsBuilder::standard().build();

    // Parse the combined file (DSL + user config)
    // Enable type annotations via Extended dialect
    let dialect = Dialect {
        enable_types: starlark::syntax::DialectTypes::Enable,
        ..Dialect::Extended
    };

    let ast = AstModule::parse(
        path.as_ref().display().to_string().as_str(),
        combined_content,
        &dialect,
    )
    .map_err(|e| anyhow::anyhow!("Failed to parse Starlark file: {}", e))?;

    // Evaluate the file
    let mut eval = Evaluator::new(&module);
    let result = eval
        .eval_module(ast, &globals)
        .map_err(|e| anyhow::anyhow!("Failed to evaluate Starlark file: {}", e))?;

    // Convert Starlark value to JSON, then deserialize using serde
    let json_value = starlark_to_json(&result)?;

    // Try deserializing directly as Os first
    if let Ok(os) = serde_json::from_value::<Os>(json_value.clone()) {
        return Ok(vec![os]);
    }

    // Otherwise try as Vec<Os>
    if let Ok(os_vec) = serde_json::from_value::<Vec<Os>>(json_value.clone()) {
        return Ok(os_vec);
    }

    // Fallback to SingleOrMultiple
    let os_or_vec: SingleOrMultiple = serde_json::from_value(json_value)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize config: {}", e))?;

    Ok(os_or_vec.into())
}

/// Convert a Starlark Value to a serde_json::Value
fn starlark_to_json(value: &Value) -> Result<serde_json::Value> {
    // Handle None
    if value.is_none() {
        return Ok(serde_json::Value::Null);
    }

    // Handle bool
    if let Some(b) = value.unpack_bool() {
        return Ok(serde_json::Value::Bool(b));
    }

    // Handle int (i32)
    if let Some(i) = value.unpack_i32() {
        return Ok(serde_json::Value::Number(i.into()));
    }

    // Handle string
    if let Some(s) = value.unpack_str() {
        return Ok(serde_json::Value::String(s.to_string()));
    }

    // Handle list
    if let Some(list) = ListRef::from_value(*value) {
        let items: Result<Vec<serde_json::Value>> = list
            .iter()
            .map(|item| starlark_to_json(&item))
            .collect();
        return Ok(serde_json::Value::Array(items?));
    }

    // Handle dict
    if let Some(dict) = DictRef::from_value(*value) {
        let mut map = serde_json::Map::new();
        for (k, v) in dict.iter() {
            let key = k
                .unpack_str()
                .ok_or_else(|| anyhow::anyhow!("Dict keys must be strings"))?;
            map.insert(key.to_string(), starlark_to_json(&v)?);
        }
        return Ok(serde_json::Value::Object(map));
    }

    // Fallback: try to convert to string
    Ok(serde_json::Value::String(value.to_str()))
}
