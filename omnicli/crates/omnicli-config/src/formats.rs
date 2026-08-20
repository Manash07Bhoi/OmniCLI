use serde_json::Value;
use std::path::Path;

use crate::error::ConfigError;

/// Format of the configuration file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    Json,
    Yaml,
    Toml,
    Xml,
    Ini,
}

impl ConfigFormat {
    pub fn from_path(path: &Path) -> Result<Self, ConfigError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        Self::parse_str(&ext).ok_or_else(|| ConfigError::UnsupportedFormat { fmt: ext })
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "xml" => Some(Self::Xml),
            "ini" | "cfg" | "conf" => Some(Self::Ini),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Xml => "xml",
            Self::Ini => "ini",
        }
    }
}

/// Parse a config file into a serde_json Value tree.
pub fn read_config(path: &Path) -> Result<Value, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::NotFound {
            path: path.display().to_string(),
        });
    }
    let content = std::fs::read_to_string(path)?;
    let fmt = ConfigFormat::from_path(path)?;
    parse_content(&content, fmt, &path.display().to_string())
}

pub fn parse_content(content: &str, fmt: ConfigFormat, path: &str) -> Result<Value, ConfigError> {
    match fmt {
        ConfigFormat::Json => serde_json::from_str(content).map_err(|e| ConfigError::Parse {
            path: path.to_owned(),
            message: e.to_string(),
        }),
        ConfigFormat::Yaml => serde_yaml::from_str(content).map_err(|e| ConfigError::Parse {
            path: path.to_owned(),
            message: e.to_string(),
        }),
        ConfigFormat::Toml => {
            let t: toml::Value = toml::from_str(content).map_err(|e| ConfigError::Parse {
                path: path.to_owned(),
                message: e.to_string(),
            })?;
            // Convert TOML → JSON Value
            serde_json::to_value(&t).map_err(|e| ConfigError::Parse {
                path: path.to_owned(),
                message: e.to_string(),
            })
        }
        ConfigFormat::Ini => parse_ini(content, path),
        ConfigFormat::Xml => parse_xml(content, path),
    }
}

/// Write a JSON Value tree back to a config file in its native format.
pub fn write_config(path: &Path, value: &Value) -> Result<(), ConfigError> {
    let fmt = ConfigFormat::from_path(path)?;
    let content = serialise_value(value, fmt)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub fn serialise_value(value: &Value, fmt: ConfigFormat) -> Result<String, ConfigError> {
    match fmt {
        ConfigFormat::Json => {
            serde_json::to_string_pretty(value).map_err(|e| ConfigError::Serialise {
                message: e.to_string(),
            })
        }
        ConfigFormat::Yaml => serde_yaml::to_string(value).map_err(|e| ConfigError::Serialise {
            message: e.to_string(),
        }),
        ConfigFormat::Toml => {
            let t: toml::Value =
                serde_json::from_value(value.clone()).map_err(|e| ConfigError::Serialise {
                    message: e.to_string(),
                })?;
            toml::to_string_pretty(&t).map_err(|e| ConfigError::Serialise {
                message: e.to_string(),
            })
        }
        ConfigFormat::Ini => serialise_ini(value),
        ConfigFormat::Xml => serialise_xml(value),
    }
}

// ── INI ──────────────────────────────────────────────────────────────────────

fn parse_ini(content: &str, path: &str) -> Result<Value, ConfigError> {
    let mut root = serde_json::Map::new();
    let mut current_section = String::new();

    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].trim().to_owned();
            root.entry(current_section.clone())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
        } else if let Some((k, v)) = line.split_once('=') {
            let key = k.trim().to_owned();
            let val = Value::String(v.trim().to_owned());
            if current_section.is_empty() {
                root.insert(key, val);
            } else {
                root.entry(current_section.clone())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()))
                    .as_object_mut()
                    .ok_or_else(|| ConfigError::Parse {
                        path: path.to_owned(),
                        message: format!("Line {}: section is not an object", line_no + 1),
                    })?
                    .insert(key, val);
            }
        }
    }
    Ok(Value::Object(root))
}

fn serialise_ini(value: &Value) -> Result<String, ConfigError> {
    let obj = value.as_object().ok_or_else(|| ConfigError::Serialise {
        message: "INI root must be an object".to_owned(),
    })?;
    let mut out = String::new();
    // Top-level scalar keys first (global section)
    for (k, v) in obj {
        if !v.is_object() {
            out.push_str(&format!("{} = {}\n", k, value_to_ini_scalar(v)));
        }
    }
    // Sections
    for (section, v) in obj {
        if let Some(section_obj) = v.as_object() {
            out.push_str(&format!("\n[{section}]\n"));
            for (k, sv) in section_obj {
                out.push_str(&format!("{} = {}\n", k, value_to_ini_scalar(sv)));
            }
        }
    }
    Ok(out)
}

fn value_to_ini_scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

// ── XML ───────────────────────────────────────────────────────────────────────

fn parse_xml(content: &str, path: &str) -> Result<Value, ConfigError> {
    // Parse XML into a simple nested JSON object where element names are keys
    // and text content is the value.
    use quick_xml::{escape::unescape, events::Event, Reader};
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut stack: Vec<(String, serde_json::Map<String, Value>)> = vec![];
    let mut root: Option<Value> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                stack.push((name, serde_json::Map::new()));
            }
            Ok(Event::Text(e)) => {
                let text_str = String::from_utf8_lossy(e.as_ref());
                let text = unescape(&text_str)
                    .map_err(|e| ConfigError::Parse {
                        path: path.to_owned(),
                        message: e.to_string(),
                    })?
                    .into_owned();
                if let Some((_, map)) = stack.last_mut() {
                    if !text.trim().is_empty() {
                        map.insert("#text".to_owned(), Value::String(text));
                    }
                }
            }
            Ok(Event::End(_)) => {
                if let Some((name, map)) = stack.pop() {
                    let val = if map.len() == 1 && map.contains_key("#text") {
                        map.into_iter()
                            .next()
                            .map(|(_, v)| v)
                            .unwrap_or(Value::Null)
                    } else if map.is_empty() {
                        Value::Null
                    } else {
                        Value::Object(map)
                    };
                    if let Some((_, parent)) = stack.last_mut() {
                        parent.insert(name, val);
                    } else {
                        root = Some(val);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ConfigError::Parse {
                    path: path.to_owned(),
                    message: e.to_string(),
                })
            }
            _ => {}
        }
    }

    Ok(root.unwrap_or(Value::Null))
}

fn serialise_xml(value: &Value) -> Result<String, ConfigError> {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>\n");
    write_xml_value(&mut out, value, 1);
    out.push_str("</root>\n");
    Ok(out)
}

fn write_xml_value(out: &mut String, value: &Value, depth: usize) {
    let indent = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                out.push_str(&format!("{indent}<{k}>"));
                match v {
                    Value::Object(_) | Value::Array(_) => {
                        out.push('\n');
                        write_xml_value(out, v, depth + 1);
                        out.push_str(&format!("{indent}</{k}>\n"));
                    }
                    other => {
                        out.push_str(&xml_escape(&scalar_to_string(other)));
                        out.push_str(&format!("</{k}>\n"));
                    }
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                out.push_str(&format!("{indent}<item>"));
                match item {
                    Value::Object(_) | Value::Array(_) => {
                        out.push('\n');
                        write_xml_value(out, item, depth + 1);
                        out.push_str(&format!("{indent}</item>\n"));
                    }
                    other => {
                        out.push_str(&xml_escape(&scalar_to_string(other)));
                        out.push_str("</item>\n");
                    }
                }
            }
        }
        other => {
            out.push_str(&xml_escape(&scalar_to_string(other)));
        }
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
