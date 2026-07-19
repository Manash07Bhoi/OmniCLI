#[cfg(test)]
mod tests {
    use std::io::Write;
    use tempfile::NamedTempFile;
    use crate::{get_key, set_key, show_config, validate_config, ConfigFormat};
    use crate::formats::{parse_content, serialise_value};

    fn write_temp(content: &str, ext: &str) -> NamedTempFile {
        let mut f = tempfile::Builder::new()
            .suffix(&format!(".{ext}"))
            .tempfile()
            .unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    // ── Format detection ──────────────────────────────────────────────────────

    #[test]
    fn detect_json_format() {
        let f = write_temp(r#"{"a": 1}"#, "json");
        let fmt = ConfigFormat::from_path(f.path()).unwrap();
        assert_eq!(fmt.as_str(), "json");
    }

    #[test]
    fn detect_toml_format() {
        let f = write_temp("[section]\na = 1\n", "toml");
        let fmt = ConfigFormat::from_path(f.path()).unwrap();
        assert_eq!(fmt.as_str(), "toml");
    }

    #[test]
    fn detect_yaml_format() {
        let f = write_temp("a: 1\nb: 2\n", "yaml");
        let fmt = ConfigFormat::from_path(f.path()).unwrap();
        assert_eq!(fmt.as_str(), "yaml");
    }

    #[test]
    fn unknown_extension_returns_error() {
        let f = write_temp("data", "bin");
        let r = ConfigFormat::from_path(f.path());
        assert!(r.is_err());
    }

    // ── show_config ───────────────────────────────────────────────────────────

    #[test]
    fn show_config_json() {
        let f = write_temp(r#"{"host": "localhost", "port": 5432}"#, "json");
        let r = show_config(f.path()).unwrap();
        assert_eq!(r.format, "json");
        assert_eq!(r.content["host"], "localhost");
        assert_eq!(r.content["port"], 5432);
    }

    #[test]
    fn show_config_toml() {
        let f = write_temp("[database]\nhost = \"localhost\"\nport = 5432\n", "toml");
        let r = show_config(f.path()).unwrap();
        assert_eq!(r.format, "toml");
        assert_eq!(r.content["database"]["host"], "localhost");
    }

    #[test]
    fn show_config_yaml() {
        let f = write_temp("host: localhost\nport: 5432\n", "yaml");
        let r = show_config(f.path()).unwrap();
        assert_eq!(r.format, "yaml");
        assert_eq!(r.content["host"], "localhost");
    }

    // ── get_key ───────────────────────────────────────────────────────────────

    #[test]
    fn get_nested_key_json() {
        let f = write_temp(r#"{"db": {"host": "localhost"}}"#, "json");
        let val = get_key(f.path(), "db.host").unwrap();
        assert_eq!(val, "localhost");
    }

    #[test]
    fn get_top_level_key() {
        let f = write_temp(r#"{"version": 2}"#, "json");
        let val = get_key(f.path(), "version").unwrap();
        assert_eq!(val, 2);
    }

    #[test]
    fn get_missing_key_returns_error() {
        let f = write_temp(r#"{"a": 1}"#, "json");
        let r = get_key(f.path(), "b");
        assert!(r.is_err());
    }

    // ── set_key ───────────────────────────────────────────────────────────────

    #[test]
    fn set_key_updates_value_json() {
        let f = write_temp(r#"{"host": "old"}"#, "json");
        set_key(f.path(), "host", "\"new-host\"").unwrap();
        let val = get_key(f.path(), "host").unwrap();
        assert_eq!(val, "new-host");
    }

    #[test]
    fn set_key_creates_nested_path() {
        let f = write_temp(r#"{}"#, "json");
        set_key(f.path(), "db.host", "\"localhost\"").unwrap();
        let val = get_key(f.path(), "db.host").unwrap();
        assert_eq!(val, "localhost");
    }

    #[test]
    fn set_key_numeric_value() {
        let f = write_temp(r#"{"port": 0}"#, "json");
        set_key(f.path(), "port", "8080").unwrap();
        let val = get_key(f.path(), "port").unwrap();
        assert_eq!(val, 8080);
    }

    // ── validate_config ───────────────────────────────────────────────────────

    #[test]
    fn validate_valid_json() {
        let f = write_temp(r#"{"ok": true}"#, "json");
        let r = validate_config(f.path());
        assert!(r.valid);
        assert!(r.error.is_none());
    }

    #[test]
    fn validate_invalid_json() {
        let f = write_temp("{not json}", "json");
        let r = validate_config(f.path());
        assert!(!r.valid);
        assert!(r.error.is_some());
    }

    #[test]
    fn validate_valid_toml() {
        let f = write_temp("[section]\nkey = \"value\"\n", "toml");
        let r = validate_config(f.path());
        assert!(r.valid);
    }

    // ── parse_content / serialise_value ───────────────────────────────────────

    #[test]
    fn parse_json_content() {
        let v = parse_content(r#"{"x": 1}"#, &ConfigFormat::Json).unwrap();
        assert_eq!(v["x"], 1);
    }

    #[test]
    fn roundtrip_json() {
        let input = r#"{"key":"value"}"#;
        let parsed = parse_content(input, &ConfigFormat::Json).unwrap();
        let out = serialise_value(&parsed, &ConfigFormat::Json).unwrap();
        let reparsed = parse_content(&out, &ConfigFormat::Json).unwrap();
        assert_eq!(parsed, reparsed);
    }
}
