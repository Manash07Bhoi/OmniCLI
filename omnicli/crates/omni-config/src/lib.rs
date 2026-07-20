pub mod error;
pub mod formats;

#[cfg(test)]
mod tests;

pub use error::ConfigError;
pub use formats::{parse_content, read_config, serialise_value, write_config, ConfigFormat};

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct ShowResult {
    pub path: String,
    pub format: String,
    pub content: Value,
}

/// Read and return a config file's parsed content.
pub fn show_config(path: &std::path::Path) -> Result<ShowResult, ConfigError> {
    let fmt = ConfigFormat::from_path(path)?;
    let content = read_config(path)?;
    Ok(ShowResult {
        path: path.display().to_string(),
        format: fmt.as_str().to_owned(),
        content,
    })
}

/// Get a value at a dotted key path, e.g. `database.host`.
pub fn get_key(path: &std::path::Path, key: &str) -> Result<Value, ConfigError> {
    let mut value = read_config(path)?;
    for part in key.split('.') {
        value = value
            .get(part)
            .ok_or_else(|| ConfigError::KeyNotFound {
                key: key.to_owned(),
            })?
            .clone();
    }
    Ok(value)
}

/// Set a value at a dotted key path and write back to the file.
pub fn set_key(path: &std::path::Path, key: &str, raw_value: &str) -> Result<(), ConfigError> {
    let mut root = read_config(path)?;
    let parts: Vec<&str> = key.split('.').collect();

    // Parse raw_value as JSON, falling back to string
    let new_val: Value =
        serde_json::from_str(raw_value).unwrap_or_else(|_| Value::String(raw_value.to_owned()));

    // Navigate to the parent and set the final key
    fn set_nested(v: &mut Value, keys: &[&str], val: Value) -> Result<(), ConfigError> {
        if keys.is_empty() {
            return Ok(());
        }
        if keys.len() == 1 {
            if let Value::Object(map) = v {
                map.insert(keys[0].to_owned(), val);
                return Ok(());
            }
            return Err(ConfigError::KeyNotFound {
                key: keys[0].to_owned(),
            });
        }
        match v {
            Value::Object(map) => {
                let next = map
                    .entry(keys[0].to_owned())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()));
                set_nested(next, &keys[1..], val)
            }
            _ => Err(ConfigError::KeyNotFound {
                key: keys[0].to_owned(),
            }),
        }
    }

    set_nested(&mut root, &parts, new_val)?;
    write_config(path, &root)
}

/// Validate a config file — parse it and return whether it's valid.
#[derive(Debug, Serialize)]
pub struct ValidateResult {
    pub path: String,
    pub format: String,
    pub valid: bool,
    pub error: Option<String>,
}

pub fn validate_config(path: &std::path::Path) -> ValidateResult {
    let path_str = path.display().to_string();
    let fmt = match ConfigFormat::from_path(path) {
        Ok(f) => f.as_str().to_owned(),
        Err(e) => {
            return ValidateResult {
                path: path_str,
                format: "unknown".to_owned(),
                valid: false,
                error: Some(e.to_string()),
            }
        }
    };
    match read_config(path) {
        Ok(_) => ValidateResult {
            path: path_str,
            format: fmt,
            valid: true,
            error: None,
        },
        Err(e) => ValidateResult {
            path: path_str,
            format: fmt,
            valid: false,
            error: Some(e.to_string()),
        },
    }
}
