use std::io::Read;

use anyhow::{Context, Result};
use serde_json::{Map, Value};

/// Builds a JSON object from optional fields, dropping the absent ones so
/// PATCH bodies only carry what the caller set.
pub fn object(fields: Vec<(&str, Option<Value>)>) -> Value {
    let mut map = Map::new();
    for (key, value) in fields {
        if let Some(value) = value {
            map.insert(key.to_string(), value);
        }
    }
    Value::Object(map)
}

pub fn string(value: &str) -> Option<Value> {
    Some(Value::String(value.to_string()))
}

pub fn opt_string(value: &Option<String>) -> Option<Value> {
    value.as_ref().map(|v| Value::String(v.clone()))
}

pub fn opt_bool(value: &Option<bool>) -> Option<Value> {
    value.map(Value::Bool)
}

pub fn strings(values: &[String]) -> Value {
    Value::Array(values.iter().map(|v| Value::String(v.clone())).collect())
}

/// Reads a JSON document from a file path, or from stdin when the path is
/// `-`.
pub fn read_json_input(path: &str) -> Result<Value> {
    let raw = if path == "-" {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .context("Could not read stdin")?;
        buffer
    } else {
        std::fs::read_to_string(path).with_context(|| format!("Could not read {path}"))?
    };
    serde_json::from_str(&raw).with_context(|| format!("{path} is not valid JSON"))
}

/// Reads a value from stdin without echoing assumptions: one trimmed line.
pub fn read_line_from_stdin(label: &str) -> Result<String> {
    use std::io::Write;
    eprint!("{label}: ");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .context("Could not read stdin")?;
    Ok(line.trim().to_string())
}
