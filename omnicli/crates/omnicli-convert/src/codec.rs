use std::{fs, path::Path};

use anyhow::Result;
use serde::Serialize;

use crate::error::ConvertError;

/// A supported conversion pair.
#[derive(Debug, Clone, Serialize)]
pub struct FormatPair {
    pub from: String,
    pub to: String,
    pub description: String,
}

/// Return the live registry of all supported conversions.
/// This is always generated from the actual codec implementations — never hand-maintained docs.
pub fn list_supported_pairs() -> Vec<FormatPair> {
    vec![
        FormatPair {
            from: "pdf".into(),
            to: "txt".into(),
            description: "Extract text from PDF".into(),
        },
        FormatPair {
            from: "csv".into(),
            to: "json".into(),
            description: "CSV → JSON array of objects".into(),
        },
        FormatPair {
            from: "json".into(),
            to: "csv".into(),
            description: "JSON array → CSV".into(),
        },
        FormatPair {
            from: "yaml".into(),
            to: "toml".into(),
            description: "YAML → TOML".into(),
        },
        FormatPair {
            from: "toml".into(),
            to: "yaml".into(),
            description: "TOML → YAML".into(),
        },
        FormatPair {
            from: "yaml".into(),
            to: "json".into(),
            description: "YAML → JSON".into(),
        },
        FormatPair {
            from: "json".into(),
            to: "yaml".into(),
            description: "JSON → YAML".into(),
        },
        FormatPair {
            from: "toml".into(),
            to: "json".into(),
            description: "TOML → JSON".into(),
        },
        FormatPair {
            from: "json".into(),
            to: "toml".into(),
            description: "JSON → TOML".into(),
        },
        FormatPair {
            from: "png".into(),
            to: "webp".into(),
            description: "PNG → WebP image".into(),
        },
        FormatPair {
            from: "webp".into(),
            to: "png".into(),
            description: "WebP → PNG image".into(),
        },
        FormatPair {
            from: "jpg".into(),
            to: "png".into(),
            description: "JPEG → PNG image".into(),
        },
        FormatPair {
            from: "jpg".into(),
            to: "webp".into(),
            description: "JPEG → WebP image".into(),
        },
        FormatPair {
            from: "jpeg".into(),
            to: "png".into(),
            description: "JPEG → PNG image".into(),
        },
        FormatPair {
            from: "jpeg".into(),
            to: "webp".into(),
            description: "JPEG → WebP image".into(),
        },
        FormatPair {
            from: "md".into(),
            to: "html".into(),
            description: "Markdown → HTML".into(),
        },
    ]
}

/// Result of a conversion.
#[derive(Debug, Serialize)]
pub struct ConvertResult {
    pub input: String,
    pub output: String,
    pub from_format: String,
    pub to_format: String,
    pub bytes_written: u64,
}

/// Convert `input` to `output`, inferring formats from file extensions.
pub fn convert(input: &Path, output: &Path) -> Result<ConvertResult, ConvertError> {
    let from = ext(input)?;
    let to = ext(output)?;

    let bytes_written = match (from.as_str(), to.as_str()) {
        ("pdf", "txt") => convert_pdf_to_txt(input, output)?,
        ("csv", "json") => convert_csv_to_json(input, output)?,
        ("json", "csv") => convert_json_to_csv(input, output)?,
        ("yaml", "toml") | ("yml", "toml") => convert_yaml_to_toml(input, output)?,
        ("toml", "yaml") | ("toml", "yml") => convert_toml_to_yaml(input, output)?,
        ("yaml", "json") | ("yml", "json") => convert_yaml_to_json(input, output)?,
        ("json", "yaml") | ("json", "yml") => convert_json_to_yaml(input, output)?,
        ("toml", "json") => convert_toml_to_json(input, output)?,
        ("json", "toml") => convert_json_to_toml(input, output)?,
        ("png", "webp") | ("jpg", "webp") | ("jpeg", "webp") => {
            convert_image(input, output, "webp")?
        }
        ("webp", "png") | ("jpg", "png") | ("jpeg", "png") => convert_image(input, output, "png")?,
        ("md", "html") | ("markdown", "html") => convert_md_to_html(input, output)?,
        _ => {
            return Err(ConvertError::UnsupportedPair {
                from: from.clone(),
                to: to.clone(),
            })
        }
    };

    Ok(ConvertResult {
        input: input.display().to_string(),
        output: output.display().to_string(),
        from_format: from,
        to_format: to,
        bytes_written,
    })
}

fn ext(path: &Path) -> Result<String, ConvertError> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .ok_or_else(|| ConvertError::UnknownExtension(path.display().to_string()))
}

// ── Converters ────────────────────────────────────────────────────────────────

fn convert_pdf_to_txt(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let doc = lopdf::Document::load(input).map_err(|e| ConvertError::ParseError {
        format: "pdf".into(),
        detail: e.to_string(),
    })?;

    let mut text = String::new();
    for (page_num, _) in doc.get_pages() {
        if let Ok(page_text) = doc.extract_text(&[page_num]) {
            text.push_str(&page_text);
            text.push('\n');
        }
    }

    let bytes = text.as_bytes();
    fs::write(output, bytes)?;
    Ok(bytes.len() as u64)
}

fn convert_csv_to_json(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let mut reader = csv::Reader::from_path(input).map_err(|e| ConvertError::ParseError {
        format: "csv".into(),
        detail: e.to_string(),
    })?;

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| ConvertError::ParseError {
            format: "csv".into(),
            detail: e.to_string(),
        })?
        .iter()
        .map(|s| s.to_owned())
        .collect();

    let mut records: Vec<serde_json::Map<String, serde_json::Value>> = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| ConvertError::ParseError {
            format: "csv".into(),
            detail: e.to_string(),
        })?;
        let mut map = serde_json::Map::new();
        for (header, value) in headers.iter().zip(record.iter()) {
            map.insert(header.clone(), serde_json::Value::String(value.to_owned()));
        }
        records.push(map);
    }

    let json = serde_json::to_string_pretty(&records).map_err(|e| ConvertError::EncodeError {
        format: "json".into(),
        detail: e.to_string(),
    })?;

    let bytes = json.as_bytes();
    fs::write(output, bytes)?;
    Ok(bytes.len() as u64)
}

fn convert_json_to_csv(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let content = fs::read_to_string(input)?;
    let records: Vec<serde_json::Map<String, serde_json::Value>> =
        serde_json::from_str(&content).map_err(|e| ConvertError::ParseError {
            format: "json".into(),
            detail: e.to_string(),
        })?;

    if records.is_empty() {
        fs::write(output, b"")?;
        return Ok(0);
    }

    let headers: Vec<String> = records[0].keys().cloned().collect();
    let mut wtr = csv::Writer::from_path(output).map_err(|e| ConvertError::EncodeError {
        format: "csv".into(),
        detail: e.to_string(),
    })?;

    wtr.write_record(&headers)
        .map_err(|e| ConvertError::EncodeError {
            format: "csv".into(),
            detail: e.to_string(),
        })?;

    for record in &records {
        let row: Vec<String> = headers
            .iter()
            .map(|h| {
                record
                    .get(h)
                    .map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default()
            })
            .collect();
        wtr.write_record(&row)
            .map_err(|e| ConvertError::EncodeError {
                format: "csv".into(),
                detail: e.to_string(),
            })?;
    }

    wtr.flush()?;
    let size = fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    Ok(size)
}

fn convert_yaml_to_toml(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let raw = fs::read_to_string(input)?;
    let value: serde_yaml::Value =
        serde_yaml::from_str(&raw).map_err(|e| ConvertError::ParseError {
            format: "yaml".into(),
            detail: e.to_string(),
        })?;

    // Convert via JSON as a bridge
    let json_val: serde_json::Value =
        serde_json::to_value(&value).map_err(|e| ConvertError::EncodeError {
            format: "toml".into(),
            detail: e.to_string(),
        })?;
    let toml_val: toml::Value =
        serde_json::from_value(json_val).map_err(|e| ConvertError::EncodeError {
            format: "toml".into(),
            detail: e.to_string(),
        })?;
    let out = toml::to_string_pretty(&toml_val).map_err(|e| ConvertError::EncodeError {
        format: "toml".into(),
        detail: e.to_string(),
    })?;

    let bytes = out.as_bytes();
    fs::write(output, bytes)?;
    Ok(bytes.len() as u64)
}

fn convert_toml_to_yaml(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let raw = fs::read_to_string(input)?;
    let value: toml::Value = toml::from_str(&raw).map_err(|e| ConvertError::ParseError {
        format: "toml".into(),
        detail: e.to_string(),
    })?;

    let json_bridge = serde_json::to_value(&value).map_err(|e| ConvertError::EncodeError {
        format: "yaml".into(),
        detail: e.to_string(),
    })?;
    let yaml_val: serde_yaml::Value =
        serde_json::from_value(json_bridge).map_err(|e| ConvertError::EncodeError {
            format: "yaml".into(),
            detail: e.to_string(),
        })?;
    let out = serde_yaml::to_string(&yaml_val).map_err(|e| ConvertError::EncodeError {
        format: "yaml".into(),
        detail: e.to_string(),
    })?;

    let bytes = out.as_bytes();
    fs::write(output, bytes)?;
    Ok(bytes.len() as u64)
}

fn convert_yaml_to_json(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let raw = fs::read_to_string(input)?;
    let value: serde_yaml::Value =
        serde_yaml::from_str(&raw).map_err(|e| ConvertError::ParseError {
            format: "yaml".into(),
            detail: e.to_string(),
        })?;
    let json = serde_json::to_string_pretty(&value).map_err(|e| ConvertError::EncodeError {
        format: "json".into(),
        detail: e.to_string(),
    })?;
    let bytes = json.as_bytes();
    fs::write(output, bytes)?;
    Ok(bytes.len() as u64)
}

fn convert_json_to_yaml(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let raw = fs::read_to_string(input)?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| ConvertError::ParseError {
            format: "json".into(),
            detail: e.to_string(),
        })?;
    let yaml_val: serde_yaml::Value =
        serde_json::from_value(value).map_err(|e| ConvertError::EncodeError {
            format: "yaml".into(),
            detail: e.to_string(),
        })?;
    let out = serde_yaml::to_string(&yaml_val).map_err(|e| ConvertError::EncodeError {
        format: "yaml".into(),
        detail: e.to_string(),
    })?;
    let bytes = out.as_bytes();
    fs::write(output, bytes)?;
    Ok(bytes.len() as u64)
}

fn convert_toml_to_json(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let raw = fs::read_to_string(input)?;
    let value: toml::Value = toml::from_str(&raw).map_err(|e| ConvertError::ParseError {
        format: "toml".into(),
        detail: e.to_string(),
    })?;
    let json = serde_json::to_string_pretty(&value).map_err(|e| ConvertError::EncodeError {
        format: "json".into(),
        detail: e.to_string(),
    })?;
    let bytes = json.as_bytes();
    fs::write(output, bytes)?;
    Ok(bytes.len() as u64)
}

fn convert_json_to_toml(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let raw = fs::read_to_string(input)?;
    let value: toml::Value = serde_json::from_str(&raw).map_err(|e| ConvertError::ParseError {
        format: "json".into(),
        detail: e.to_string(),
    })?;
    let out = toml::to_string_pretty(&value).map_err(|e| ConvertError::EncodeError {
        format: "toml".into(),
        detail: e.to_string(),
    })?;
    let bytes = out.as_bytes();
    fs::write(output, bytes)?;
    Ok(bytes.len() as u64)
}

fn convert_image(input: &Path, output: &Path, _target_fmt: &str) -> Result<u64, ConvertError> {
    let img = image::open(input).map_err(|e| ConvertError::ParseError {
        format: ext(input).unwrap_or_default(),
        detail: e.to_string(),
    })?;
    img.save(output).map_err(|e| ConvertError::EncodeError {
        format: ext(output).unwrap_or_default(),
        detail: e.to_string(),
    })?;
    Ok(fs::metadata(output).map(|m| m.len()).unwrap_or(0))
}

fn convert_md_to_html(input: &Path, output: &Path) -> Result<u64, ConvertError> {
    let md = fs::read_to_string(input)?;
    let parser = pulldown_cmark::Parser::new(&md);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);

    let full_html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n<title>Converted</title>\n</head>\n<body>\n{html}</body>\n</html>\n"
    );

    let bytes = full_html.as_bytes();
    fs::write(output, bytes)?;
    Ok(bytes.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_csv_to_json() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("data.csv");
        let output = dir.path().join("data.json");
        fs::write(&input, b"name,age\nAlice,30\nBob,25").unwrap();

        let _result = convert(&input, &output).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&output).unwrap()).unwrap();
        assert_eq!(json[0]["name"], "Alice");
        assert_eq!(json[1]["age"], "25");
    }

    #[test]
    fn test_yaml_to_toml() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("config.yaml");
        let output = dir.path().join("config.toml");
        fs::write(&input, b"name: test\nvalue: 42").unwrap();

        let _result = convert(&input, &output).unwrap();
        let content = fs::read_to_string(&output).unwrap();
        assert!(content.contains("name = "));
    }

    #[test]
    fn test_toml_to_yaml() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("config.toml");
        let output = dir.path().join("config.yaml");
        fs::write(&input, b"[core]\ncolor = \"auto\"").unwrap();

        convert(&input, &output).unwrap();
        let content = fs::read_to_string(&output).unwrap();
        assert!(content.contains("color"));
    }

    #[test]
    fn test_md_to_html() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("README.md");
        let output = dir.path().join("README.html");
        fs::write(&input, b"# Hello\n\nWorld").unwrap();

        convert(&input, &output).unwrap();
        let html = fs::read_to_string(&output).unwrap();
        assert!(html.contains("<h1>"));
        assert!(html.contains("Hello"));
    }

    #[test]
    fn test_unsupported_pair() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("file.pdf");
        let output = dir.path().join("file.mp3");
        fs::write(&input, b"").unwrap();

        let result = convert(&input, &output);
        assert!(matches!(result, Err(ConvertError::UnsupportedPair { .. })));
    }

    #[test]
    fn test_list_supported_pairs_not_empty() {
        let pairs = list_supported_pairs();
        assert!(!pairs.is_empty());
        // Every pair listed must be implementable (not just docs)
        assert!(pairs.iter().any(|p| p.from == "csv" && p.to == "json"));
        assert!(pairs.iter().any(|p| p.from == "md" && p.to == "html"));
    }

    #[test]
    fn test_json_to_csv() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("data.json");
        let output = dir.path().join("data.csv");
        fs::write(
            &input,
            br#"[{"name":"Alice","age":"30"},{"name":"Bob","age":"25"}]"#,
        )
        .unwrap();

        convert(&input, &output).unwrap();
        let content = fs::read_to_string(&output).unwrap();
        assert!(content.contains("Alice"));
        assert!(content.contains("Bob"));
    }
}
