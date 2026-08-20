#[cfg(test)]
mod dev_tests {
    use crate::{
        base64::process_base64, compute_hash, decode_jwt, generate_uuids, process_json, test_regex,
    };

    // ── hash ──────────────────────────────────────────────────────────────────

    #[test]
    fn hash_sha256_empty_string() {
        let r = compute_hash("", "sha256").unwrap();
        // NIST test vector: SHA-256 of ""
        assert_eq!(
            r.digest,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(r.algo, "sha256");
        assert_eq!(r.input_len, 0);
    }

    #[test]
    fn hash_sha256_hello() {
        let r = compute_hash("hello", "sha256").unwrap();
        assert_eq!(
            r.digest,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn hash_md5_hello() {
        let r = compute_hash("hello", "md5").unwrap();
        assert_eq!(r.digest, "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn hash_sha1_hello() {
        let r = compute_hash("hello", "sha1").unwrap();
        assert_eq!(r.digest, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn hash_blake3_hello() {
        let r = compute_hash("hello", "blake3").unwrap();
        // blake3 of "hello" — fixed reference value
        assert_eq!(
            r.digest,
            "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f"
        );
    }

    #[test]
    fn hash_unsupported_algo_returns_error() {
        let r = compute_hash("hello", "crc32");
        assert!(r.is_err(), "unsupported algo should return Err");
    }

    #[test]
    fn hash_long_input_truncated_in_display() {
        let long = "a".repeat(200);
        let r = compute_hash(&long, "sha256").unwrap();
        assert!(
            r.input.contains('…'),
            "long input should be truncated with ellipsis"
        );
        assert_eq!(r.input_len, 200);
    }

    // ── base64 ────────────────────────────────────────────────────────────────

    #[test]
    fn base64_encode_hello() {
        let r = process_base64("hello", false).unwrap();
        assert_eq!(r.output, "aGVsbG8=");
    }

    #[test]
    fn base64_decode_hello() {
        let r = process_base64("aGVsbG8=", true).unwrap();
        assert_eq!(r.output, "hello");
    }

    #[test]
    fn base64_roundtrip() {
        let original = "OmniCLI rocks! 🚀";
        let encoded = process_base64(original, false).unwrap();
        let decoded = process_base64(&encoded.output, true).unwrap();
        assert_eq!(decoded.output, original);
    }

    #[test]
    fn base64_invalid_decode_returns_error() {
        // not valid base64
        let r = process_base64("!!!not-base64!!!", true);
        assert!(r.is_err());
    }

    #[test]
    // ── json ──────────────────────────────────────────────────────────────────

    fn json_format_valid() {
        let r = process_json(r#"{"a":1,"b":2}"#, "format", None).unwrap();
        // formatted output should be valid JSON
        let v: serde_json::Value = serde_json::from_str(&r.output).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn json_minify_valid() {
        let input = "{\n  \"a\": 1,\n  \"b\": 2\n}";
        let r = process_json(input, "minify", None).unwrap();
        assert!(
            !r.output.contains('\n'),
            "minified JSON should not contain newlines"
        );
    }

    #[test]
    fn json_invalid_input_returns_error() {
        let r = process_json("{not json}", "format", None);
        assert!(r.is_err());
    }

    // ── uuid ──────────────────────────────────────────────────────────────────

    #[test]
    fn uuid_generates_correct_count() {
        let uuids = generate_uuids(5, "v4").unwrap();
        assert_eq!(uuids.uuids.len(), 5);
    }

    #[test]
    fn uuid_format_valid() {
        let uuids = generate_uuids(1, "v4").unwrap();
        let u = &uuids.uuids[0];
        // UUID v4: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        assert_eq!(u.len(), 36);
        let parts: Vec<&str> = u.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(
            parts[2].chars().next(),
            Some('4'),
            "UUIDv4 third group starts with 4"
        );
    }

    #[test]
    fn uuid_all_unique() {
        let uuids = generate_uuids(100, "v4").unwrap();
        let set: std::collections::HashSet<&String> = uuids.uuids.iter().collect();
        assert_eq!(set.len(), 100, "all generated UUIDs should be unique");
    }

    // ── regex ─────────────────────────────────────────────────────────────────

    #[test]
    fn regex_matches_basic() {
        let r = test_regex(r"\d+", "abc 123 def", "").unwrap();
        assert!(!r.matches.is_empty());
        assert!(!r.matches.is_empty());
        assert_eq!(r.matches[0].text, "123");
    }

    #[test]
    fn regex_no_match() {
        let r = test_regex(r"\d+", "no digits here", "").unwrap();
        assert!(r.matches.is_empty());
        assert!(r.matches.is_empty());
    }

    #[test]
    fn regex_multiple_matches() {
        let r = test_regex(r"\b\w{4}\b", "this is test code", "").unwrap();
        assert!(!r.matches.is_empty());
        assert!(r.matches.len() >= 2);
    }

    #[test]
    // ── jwt ───────────────────────────────────────────────────────────────────

    fn jwt_decodes_known_token() {
        // Unsigned test JWT: header.payload (no signature validation — decode only)
        // Header: {"alg":"HS256","typ":"JWT"}
        // Payload: {"sub":"1234567890","name":"OmniCLI","iat":1516239022}
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6Ik9tbmlDTEkiLCJpYXQiOjE1MTYyMzkwMjJ9.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let r = decode_jwt(token).unwrap();
        assert_eq!(r.payload["sub"], "1234567890");
        assert_eq!(r.payload["name"], "OmniCLI");
    }

    #[test]
    fn jwt_invalid_format_returns_error() {
        let r = decode_jwt("not-a-jwt");
        assert!(r.is_err());
    }

    #[test]
    fn jwt_too_few_parts_returns_error() {
        let r = decode_jwt("header.payload");
        assert!(r.is_err());
    }
}
