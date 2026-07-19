use serde::Serialize;
use serde_json::Value;

use crate::error::DevError;

#[derive(Debug, Serialize)]
pub struct JsonResult {
    pub valid: bool,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn process_json(input: &str, action: &str, query: Option<&str>) -> Result<JsonResult, DevError> {
    let parsed: serde_json::Result<Value> = serde_json::from_str(input);

    match action {
        "validate" => Ok(JsonResult {
            valid: parsed.is_ok(),
            output: if parsed.is_ok() {
                "Valid JSON".to_owned()
            } else {
                String::new()
            },
            error: parsed.err().map(|e| e.to_string()),
        }),
        "pretty" | "format" => {
            let v = parsed.map_err(|e| DevError::Parse {
                message: e.to_string(),
            })?;
            let v = apply_query(v, query)?;
            let output = serde_json::to_string_pretty(&v).map_err(|e| DevError::Parse {
                message: e.to_string(),
            })?;
            Ok(JsonResult { valid: true, output, error: None })
        }
        "minify" => {
            let v = parsed.map_err(|e| DevError::Parse {
                message: e.to_string(),
            })?;
            let v = apply_query(v, query)?;
            let output = serde_json::to_string(&v).map_err(|e| DevError::Parse {
                message: e.to_string(),
            })?;
            Ok(JsonResult { valid: true, output, error: None })
        }
        other => Err(DevError::InvalidInput {
            message: format!("Unknown action: {other}. Use: pretty, minify, validate"),
        }),
    }
}

/// Apply a simple dotted-path query like `.user.name` or `.items[0]`.
fn apply_query(mut v: Value, query: Option<&str>) -> Result<Value, DevError> {
    let q = match query {
        Some(q) if !q.is_empty() => q.trim_start_matches('.'),
        _ => return Ok(v),
    };

    for key in q.split('.') {
        if key.is_empty() {
            continue;
        }
        // Handle array index syntax: key[0]
        if let Some(bracket) = key.find('[') {
            let field = &key[..bracket];
            let idx_str = key[bracket + 1..].trim_end_matches(']');
            let idx: usize = idx_str.parse().map_err(|_| DevError::InvalidInput {
                message: format!("Invalid array index: {idx_str}"),
            })?;
            if !field.is_empty() {
                v = v
                    .get(field)
                    .ok_or_else(|| DevError::InvalidInput {
                        message: format!("Key not found: {field}"),
                    })?
                    .clone();
            }
            v = v
                .get(idx)
                .ok_or_else(|| DevError::InvalidInput {
                    message: format!("Index out of bounds: {idx}"),
                })?
                .clone();
        } else {
            v = v
                .get(key)
                .ok_or_else(|| DevError::InvalidInput {
                    message: format!("Key not found: {key}"),
                })?
                .clone();
        }
    }
    Ok(v)
}
