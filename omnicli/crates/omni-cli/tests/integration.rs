//! Cross-crate integration tests.
//!
//! These tests exercise real pipelines that span multiple library crates:
//! hash → compare, CSV → JSON → YAML chains, archive roundtrips,
//! encrypt → decrypt, and search index + query.

use tempfile::tempdir;

// ── Hash + compare pipeline ───────────────────────────────────────────────────

#[test]
fn test_hash_and_compare_pipeline() {
    use omni_core::hash::{hash_bytes, HashAlgo};
    use omni_file::compare::compare_files;
    use omni_file::copy_move::{copy_path, CopyOptions};

    let dir = tempdir().unwrap();
    let src = dir.path().join("source.txt");
    let dst = dir.path().join("dest.txt");
    std::fs::write(&src, b"integration test data 42").unwrap();

    // Hash the source with BLAKE3.
    let h1 = hash_bytes(b"integration test data 42", HashAlgo::Blake3);
    assert_eq!(h1.len(), 64, "BLAKE3 digest should be 64 hex chars");

    // Copy, then compare — must be identical.
    let opts = CopyOptions {
        source: src.clone(),
        dest: dst.clone(),
        recursive: false,
        verify: true,
        dry_run: false,
    };
    let result = copy_path(&opts).unwrap();
    assert_eq!(result.files_copied, 1);

    let cmp = compare_files(&src, &dst, false).unwrap();
    assert!(cmp.identical, "copied file should be identical to source");

    // Corrupt dest and confirm difference is detected.
    std::fs::write(&dst, b"CORRUPTED").unwrap();
    let cmp2 = compare_files(&src, &dst, false).unwrap();
    assert!(!cmp2.identical);
    assert!(cmp2.first_diff_offset.is_some());
}

// ── Format conversion chain ───────────────────────────────────────────────────

#[test]
fn test_conversion_chain_csv_json_yaml() {
    use omni_convert::codec::convert;

    let dir = tempdir().unwrap();
    let csv_path = dir.path().join("data.csv");
    let json_path = dir.path().join("data.json");
    let yaml_path = dir.path().join("data.yaml");

    std::fs::write(&csv_path, b"name,age,city\nAlice,30,London\nBob,25,Berlin").unwrap();

    // CSV → JSON
    let r1 = convert(&csv_path, &json_path).unwrap();
    assert_eq!(r1.from_format, "csv");
    assert_eq!(r1.to_format, "json");
    assert!(r1.bytes_written > 0);

    // Validate JSON content
    let json_content = std::fs::read_to_string(&json_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();
    assert_eq!(parsed[0]["name"], "Alice");
    assert_eq!(parsed[1]["city"], "Berlin");

    // JSON → YAML
    let r2 = convert(&json_path, &yaml_path).unwrap();
    assert_eq!(r2.from_format, "json");
    assert_eq!(r2.to_format, "yaml");

    let yaml_content = std::fs::read_to_string(&yaml_path).unwrap();
    assert!(yaml_content.contains("Alice") || yaml_content.contains("name"));
}

#[test]
fn test_conversion_toml_yaml_roundtrip() {
    use omni_convert::codec::convert;

    let dir = tempdir().unwrap();
    let toml_in = dir.path().join("config.toml");
    let yaml_mid = dir.path().join("config.yaml");
    let toml_out = dir.path().join("config2.toml");

    std::fs::write(&toml_in, b"[server]\nhost = \"localhost\"\nport = 8080\n").unwrap();

    convert(&toml_in, &yaml_mid).unwrap();
    let yaml = std::fs::read_to_string(&yaml_mid).unwrap();
    assert!(yaml.contains("host") || yaml.contains("server"));

    convert(&yaml_mid, &toml_out).unwrap();
    let toml = std::fs::read_to_string(&toml_out).unwrap();
    assert!(toml.contains("localhost") || toml.contains("host"));
}

#[test]
fn test_conversion_md_to_html() {
    use omni_convert::codec::convert;

    let dir = tempdir().unwrap();
    let md = dir.path().join("doc.md");
    let html = dir.path().join("doc.html");
    std::fs::write(
        &md,
        b"# OmniCLI\n\nOne CLI to rule them all.\n\n- file ops\n- search\n",
    )
    .unwrap();

    convert(&md, &html).unwrap();
    let content = std::fs::read_to_string(&html).unwrap();
    assert!(content.contains("<h1>"));
    assert!(content.contains("OmniCLI"));
    assert!(content.contains("<li>"));
}

// ── Archive roundtrips ────────────────────────────────────────────────────────

#[test]
fn test_archive_zip_roundtrip() {
    use omni_archive::{create_archive, extract_archive, list_archive};

    let dir = tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    std::fs::write(&a, b"file A content").unwrap();
    std::fs::write(&b, b"file B content").unwrap();

    let archive = dir.path().join("bundle.zip");
    let created = create_archive(&archive, &[a.clone(), b.clone()]).unwrap();
    assert_eq!(created.files_added, 2);
    assert_eq!(created.format, "zip");

    let entries = list_archive(&archive).unwrap();
    assert_eq!(entries.len(), 2);

    let out = dir.path().join("extracted");
    let extracted = extract_archive(&archive, Some(&out)).unwrap();
    assert_eq!(extracted.files_extracted, 2);
    assert_eq!(std::fs::read(out.join("a.txt")).unwrap(), b"file A content");
    assert_eq!(std::fs::read(out.join("b.txt")).unwrap(), b"file B content");
}

#[test]
fn test_archive_tar_gz_roundtrip() {
    use omni_archive::{create_archive, extract_archive};

    let dir = tempdir().unwrap();
    let src = dir.path().join("notes.txt");
    std::fs::write(&src, b"compressed notes here").unwrap();

    let archive = dir.path().join("notes.tar.gz");
    let created = create_archive(&archive, &[src]).unwrap();
    assert_eq!(created.format, "tar.gz");
    assert!(created.archive_size_bytes > 0);

    let out = dir.path().join("out");
    let extracted = extract_archive(&archive, Some(&out)).unwrap();
    assert_eq!(extracted.files_extracted, 1);
    assert_eq!(
        std::fs::read(out.join("notes.txt")).unwrap(),
        b"compressed notes here"
    );
}

#[test]
fn test_archive_tar_bz2_roundtrip() {
    use omni_archive::{create_archive, extract_archive, list_archive};

    let dir = tempdir().unwrap();
    let src = dir.path().join("data.txt");
    std::fs::write(&src, b"bzip2 integration test content").unwrap();

    let archive = dir.path().join("data.tar.bz2");
    create_archive(&archive, &[src]).unwrap();

    let entries = list_archive(&archive).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "data.txt");

    let out = dir.path().join("bz2_extracted");
    let result = extract_archive(&archive, Some(&out)).unwrap();
    assert_eq!(result.files_extracted, 1);
    assert_eq!(
        std::fs::read(out.join("data.txt")).unwrap(),
        b"bzip2 integration test content"
    );
}

#[test]
fn test_archive_convert_zip_to_tar_gz() {
    use omni_archive::create::create_archive as ca;
    use omni_archive::{create_archive, extract_archive};

    let dir = tempdir().unwrap();
    let src = dir.path().join("readme.txt");
    std::fs::write(&src, b"readme content for conversion test").unwrap();

    // Create a zip archive
    let zip = dir.path().join("bundle.zip");
    create_archive(&zip, &[src]).unwrap();

    // Convert: extract to tmp, repack as tar.gz
    let tmp = tempfile::tempdir().unwrap();
    extract_archive(&zip, Some(tmp.path())).unwrap();
    let tar_gz = dir.path().join("bundle.tar.gz");
    ca(&tar_gz, &[tmp.path().to_owned()]).unwrap();
    assert!(tar_gz.exists());
    assert!(tar_gz.metadata().unwrap().len() > 0);
}

// ── Encrypt → decrypt roundtrip ───────────────────────────────────────────────

#[test]
fn test_encrypt_decrypt_roundtrip() {
    use age::secrecy::ExposeSecret;
    use age::x25519::Identity;
    use omni_file::{decrypt::decrypt_file, encrypt::encrypt_file};

    let dir = tempdir().unwrap();
    let src = dir.path().join("secret.txt");
    let enc = dir.path().join("secret.txt.age");
    let dec = dir.path().join("secret_decrypted.txt");

    let payload = b"sensitive data \xe2\x80\x94 do not share";
    std::fs::write(&src, payload).unwrap();

    let identity = Identity::generate();
    let pub_key = identity.to_public().to_string();
    // age 0.10 wraps the private key in Secret<String> to prevent accidental logging.
    let priv_key_secret = identity.to_string();
    let priv_key = priv_key_secret.expose_secret();

    encrypt_file(&src, Some(&enc), &pub_key).unwrap();
    assert!(enc.exists());
    assert_ne!(std::fs::read(&enc).unwrap().as_slice(), payload);

    decrypt_file(&enc, Some(&dec), priv_key).unwrap();
    assert_eq!(std::fs::read(&dec).unwrap(), payload);
}

// ── Search: index then query ──────────────────────────────────────────────────

#[test]
fn test_search_index_and_query() {
    use omni_search::search::ContentFilter;
    use omni_search::{open_index_db, rebuild_index, search_query, SearchOptions};

    let dir = tempdir().unwrap();

    // Create corpus files
    let code_file = dir.path().join("vuln_report.rs");
    std::fs::write(
        &code_file,
        b"// CVE-2026-9999: buffer overflow in omni_parse\nfn parse() {}",
    )
    .unwrap();

    let notes_file = dir.path().join("notes.txt");
    std::fs::write(
        &notes_file,
        b"See CVE-2026-9999 for the full advisory details.",
    )
    .unwrap();

    let db_path = dir.path().join("search.db");
    let mut conn = open_index_db(&db_path).unwrap();
    let stats = rebuild_index(&mut conn, &[dir.path().to_owned()], &[], true).unwrap();
    assert!(stats.files_indexed >= 2);

    let opts = SearchOptions {
        query: "CVE-2026-9999".to_string(),
        content_filter: ContentFilter::all(),
        use_regex: false,
        case_sensitive: false,
        limit: 50,
    };
    let results = search_query(&conn, &opts).unwrap();
    assert!(!results.is_empty(), "should find at least one CVE match");
    assert!(results
        .iter()
        .any(|r| r.path.contains("vuln_report") || r.path.contains("notes")));
}

// ── Find files ────────────────────────────────────────────────────────────────

#[test]
fn test_find_files_by_pattern() {
    use omni_file::find::{find_files, EntryType, FindOptions};

    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), b"fn main() {}").unwrap();
    std::fs::write(dir.path().join("lib.rs"), b"pub fn lib() {}").unwrap();
    std::fs::write(dir.path().join("config.toml"), b"[core]\n").unwrap();

    let opts = FindOptions {
        pattern: Some(".rs".to_string()),
        regex: false,
        entry_type: EntryType::File,
        size: None,
        modified_within: None,
        path: dir.path().to_owned(),
        max_depth: None,
        follow_symlinks: false,
    };

    let entries = find_files(&opts).unwrap();
    assert_eq!(entries.len(), 2, "should find exactly 2 .rs files");
    assert!(entries.iter().all(|e| e.path.ends_with(".rs")));
}

#[test]
fn test_find_files_all_types() {
    use omni_file::find::{find_files, EntryType, FindOptions};

    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), b"hello").unwrap();
    std::fs::write(dir.path().join("b.txt"), b"world").unwrap();
    std::fs::create_dir(dir.path().join("subdir")).unwrap();

    let opts = FindOptions {
        pattern: None,
        regex: false,
        entry_type: EntryType::File,
        size: None,
        modified_within: None,
        path: dir.path().to_owned(),
        max_depth: None,
        follow_symlinks: false,
    };

    let entries = find_files(&opts).unwrap();
    assert!(entries.len() >= 2);
}

// ── Duplicate detection ───────────────────────────────────────────────────────

#[test]
fn test_duplicate_detection() {
    use omni_file::duplicate::scan_duplicates;

    let dir = tempdir().unwrap();
    let payload = b"identical content for duplicate test";
    std::fs::write(dir.path().join("copy1.txt"), payload).unwrap();
    std::fs::write(dir.path().join("copy2.txt"), payload).unwrap();
    std::fs::write(
        dir.path().join("unique.txt"),
        b"something completely different",
    )
    .unwrap();

    let result = scan_duplicates(dir.path()).unwrap();
    assert_eq!(result.groups.len(), 1, "exactly one duplicate group");
    assert_eq!(result.groups[0].files.len(), 2);
    assert!(result.wasted_bytes > 0);
}

// ── Conversion supported-pairs list ──────────────────────────────────────────

#[test]
fn test_list_supported_pairs_complete() {
    let pairs = omni_convert::codec::list_supported_pairs();
    assert!(!pairs.is_empty());

    // Every documented pair must be present
    let has = |from: &str, to: &str| pairs.iter().any(|p| p.from == from && p.to == to);
    assert!(has("csv", "json"));
    assert!(has("json", "csv"));
    assert!(has("yaml", "toml"));
    assert!(has("toml", "yaml"));
    assert!(has("yaml", "json"));
    assert!(has("json", "yaml"));
    assert!(has("toml", "json"));
    assert!(has("json", "toml"));
    assert!(has("md", "html"));
    assert!(has("pdf", "txt"));
    assert!(has("png", "webp"));
}
