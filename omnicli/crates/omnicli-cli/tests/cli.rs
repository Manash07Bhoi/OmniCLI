//! End-to-end CLI tests: they execute the real binary and assert on exit codes,
//! stdout/stderr separation, JSON payloads, and filesystem effects.
//!
//! Every test runs against a throwaway `HOME`/`XDG_*` so the developer's real
//! configuration and workspace database are never touched.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

/// A sandboxed environment: isolated HOME, XDG dirs, and working directory.
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omnicli-cli-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }
    fn work(&self) -> PathBuf {
        self.root.join("work")
    }

    fn prepare(&self) {
        fs::create_dir_all(self.home()).unwrap();
        fs::create_dir_all(self.work()).unwrap();
    }

    /// Run the CLI with the sandboxed environment.
    fn run(&self, args: &[&str]) -> Output {
        self.run_with_stdin(args, None)
    }

    fn run_with_stdin(&self, args: &[&str], stdin: Option<&str>) -> Output {
        use std::io::Write;
        use std::process::Stdio;

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_omnicli"));
        cmd.args(args)
            .current_dir(self.work())
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("XDG_DATA_HOME", self.home().join(".local/share"))
            .env("NO_COLOR", "")
            .env_remove("OMNI_INDEX_PATH")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            });

        let mut child = cmd.spawn().expect("failed to spawn omnicli");
        if let Some(text) = stdin {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(text.as_bytes())
                .unwrap();
        }
        child.wait_with_output().expect("failed to run omnicli")
    }

    fn write_config(&self, contents: &str) -> PathBuf {
        let dir = self.home().join(".config/omni");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("omnicli.toml");
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}
fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}
fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

// ── Basics ────────────────────────────────────────────────────────────────────

#[test]
fn version_and_help_work() {
    let sb = Sandbox::new();
    sb.prepare();

    let version = sb.run(&["--version"]);
    assert_eq!(code(&version), 0);
    assert!(stdout_of(&version).contains("omnicli"));

    let help = sb.run(&["--help"]);
    assert_eq!(code(&help), 0);
    let text = stdout_of(&help);
    for group in [
        "file",
        "search",
        "archive",
        "convert",
        "config",
        "dev",
        "backup",
        "workspace",
        "completions",
    ] {
        assert!(text.contains(group), "help must list `{group}`");
    }
}

#[test]
fn usage_error_exits_with_2() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run(&["definitely-not-a-command"]);
    assert_eq!(code(&out), 2, "clap usage errors must exit 2");
}

#[test]
fn completions_are_generated_on_stdout_only() {
    let sb = Sandbox::new();
    sb.prepare();
    for shell in ["bash", "zsh", "fish"] {
        let out = sb.run(&["completions", shell]);
        assert_eq!(code(&out), 0, "completions {shell} failed");
        assert!(
            !stdout_of(&out).is_empty(),
            "completions {shell} produced no output"
        );
        assert!(
            stderr_of(&out).is_empty(),
            "completions must not write to stderr"
        );
    }
}

/// The generated script must complete the name of the installed binary.
///
/// This regressed once: the script was generated for a hard-coded `omni`, so
/// `omnicli <TAB>` never completed anything after the rename.
#[test]
fn completions_register_the_real_binary_name() {
    let sb = Sandbox::new();
    sb.prepare();

    let bash = stdout_of(&sb.run(&["completions", "bash"]));
    assert!(
        bash.contains("complete -F _omnicli"),
        "bash completions must bind the `omnicli` command"
    );
    assert!(
        !bash.contains("complete -F _omni "),
        "bash completions must not bind a name that does not exist"
    );

    let zsh = stdout_of(&sb.run(&["completions", "zsh"]));
    assert!(zsh.starts_with("#compdef omnicli"), "zsh compdef mismatch");

    let fish = stdout_of(&sb.run(&["completions", "fish"]));
    assert!(
        fish.contains("complete -c omnicli"),
        "fish completions must bind the `omnicli` command"
    );

    for shell in ["elvish", "powershell"] {
        let out = sb.run(&["completions", shell]);
        assert_eq!(code(&out), 0, "completions {shell} failed");
        assert!(stdout_of(&out).contains("omnicli"));
    }
}

/// `--regex` searches file content (not just names), `--in` really filters
/// result types, and the shorthand form accepts the same flags as
/// `search query`. Regression tests for behaviour the docs already promised.
#[test]
fn search_regex_in_and_case_flags_behave() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("a.log"), "unexpected ERROR occurred\n").unwrap();
    fs::write(sb.work().join("b.rs"), "fn payload() {}\n").unwrap();
    fs::write(sb.work().join("c.db"), "x").unwrap();
    assert_eq!(code(&sb.run(&["search", "index", "."])), 0);

    // --regex matches inside a token that FTS phrase search would miss.
    let out = sb.run(&["search", "query", "err", "--regex"]);
    assert_eq!(code(&out), 0);
    assert!(
        stdout_of(&out).contains("a.log"),
        "got: {}",
        stdout_of(&out)
    );

    // --in filters results by content type.
    let out = sb.run(&["search", "query", "ERROR", "--in", "code"]);
    assert_eq!(code(&out), 0);
    assert!(
        !stdout_of(&out).contains("a.log"),
        "--in code must exclude .log, got: {}",
        stdout_of(&out)
    );

    let out = sb.run(&["search", "query", "payload", "--in", "logs"]);
    assert_eq!(code(&out), 0);
    assert!(
        !stdout_of(&out).contains("b.rs"),
        "--in logs must exclude .rs, got: {}",
        stdout_of(&out)
    );

    let out = sb.run(&["search", "query", "payload", "--in", "sqlite"]);
    assert_eq!(code(&out), 0);
    assert!(
        stderr_of(&out).contains("No results"),
        "a .db entry has no searchable content, got: {} / {}",
        stdout_of(&out),
        stderr_of(&out)
    );

    // --case-sensitive requires exact casing.
    let out = sb.run(&["search", "query", "error", "--case-sensitive"]);
    assert_eq!(code(&out), 0);
    assert!(
        !stdout_of(&out).contains("a.log"),
        "exact-case `error` must not match `ERROR`"
    );
    let out = sb.run(&["search", "query", "ERROR", "--case-sensitive"]);
    assert_eq!(code(&out), 0);
    assert!(stdout_of(&out).contains("a.log"));

    // The shorthand form takes the same flags as `search query`.
    let out = sb.run(&["search", "payload", "--in", "code", "--regex"]);
    assert_eq!(code(&out), 0);
    assert!(stdout_of(&out).contains("b.rs"), "got: {}", stdout_of(&out));
}

/// A reader that goes away early (`omnicli … | head -1`) must not panic.
#[test]
fn closed_stdout_does_not_panic() {
    use std::io::Read;
    use std::process::Stdio;

    let sb = Sandbox::new();
    sb.prepare();
    for i in 0..50 {
        fs::write(sb.work().join(format!("pipe-{i}.txt")), "x").unwrap();
    }

    for args in [
        vec!["file", "find", "*.txt", "--path", "."],
        vec!["completions", "bash"],
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omnicli"))
            .args(&args)
            .current_dir(sb.work())
            .env("HOME", sb.home())
            .env("XDG_CONFIG_HOME", sb.home().join(".config"))
            .env("XDG_DATA_HOME", sb.home().join(".local/share"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
            .expect("failed to spawn omnicli");

        // Read a little, then close the read end while the child is still writing.
        {
            let mut buf = [0u8; 64];
            let mut out = child.stdout.take().unwrap();
            let _ = out.read(&mut buf);
        }

        let mut stderr = String::new();
        child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        let status = child.wait().unwrap();
        assert_ne!(
            status.code(),
            Some(101),
            "exit code 101 is a Rust panic — `{}` must exit quietly instead",
            args.join(" ")
        );
        assert!(
            !stderr.contains("panicked"),
            "`{}` panicked on a closed pipe: {stderr}",
            args.join(" ")
        );
    }
}

// ── Configuration ─────────────────────────────────────────────────────────────

#[test]
fn explicit_missing_config_exits_with_3() {
    let sb = Sandbox::new();
    sb.prepare();
    let missing = sb.root.join("nope/omni.toml");
    let out = sb.run(&["--config", missing.to_str().unwrap(), "config", "show"]);
    assert_eq!(code(&out), 3, "a missing --config file must exit 3");
    assert!(
        stderr_of(&out).contains("Config file not found"),
        "stderr was: {}",
        stderr_of(&out)
    );
}

#[test]
fn malformed_config_exits_with_3() {
    let sb = Sandbox::new();
    sb.prepare();
    let cfg = sb.write_config("this is not toml [[[");
    let out = sb.run(&["--config", cfg.to_str().unwrap(), "config", "show"]);
    assert_eq!(code(&out), 3);
    assert!(stderr_of(&out).contains("Invalid config file"));
}

#[test]
fn invalid_config_value_exits_with_3_and_names_the_field() {
    let sb = Sandbox::new();
    sb.prepare();
    let cfg = sb.write_config("[core]\ncolor = \"purple\"\n");
    let out = sb.run(&["--config", cfg.to_str().unwrap(), "config", "show"]);
    assert_eq!(code(&out), 3);
    assert!(
        stderr_of(&out).contains("core.color"),
        "expected the offending field in stderr, got: {}",
        stderr_of(&out)
    );
}

#[test]
fn unknown_config_key_is_rejected() {
    let sb = Sandbox::new();
    sb.prepare();
    let cfg = sb.write_config("[core]\ncolr = \"never\"\n");
    let out = sb.run(&["--config", cfg.to_str().unwrap(), "config", "show"]);
    assert_eq!(code(&out), 3, "typos in config keys must not pass silently");
}

#[test]
fn config_default_hash_is_honoured() {
    let sb = Sandbox::new();
    sb.prepare();
    let cfg = sb.write_config("[file]\ndefault_hash = \"sha256\"\n");
    let file = sb.work().join("payload.txt");
    fs::write(&file, b"hello world").unwrap();

    let out = sb.run(&[
        "--config",
        cfg.to_str().unwrap(),
        "file",
        "hash",
        file.to_str().unwrap(),
    ]);
    assert_eq!(code(&out), 0);
    // SHA-256 of "hello world".
    assert!(
        stdout_of(&out)
            .contains("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"),
        "stdout was: {}",
        stdout_of(&out)
    );
}

// ── Output contract ───────────────────────────────────────────────────────────

#[test]
fn json_output_is_parseable_and_free_of_ansi() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("a.txt"), b"content").unwrap();

    let out = sb.run(&["--json", "file", "stats", "a.txt"]);
    assert_eq!(code(&out), 0);
    let text = stdout_of(&out);
    assert!(
        !text.contains('\x1b'),
        "--json must never emit ANSI: {text:?}"
    );
    let value: serde_json::Value = serde_json::from_str(&text).expect("stdout must be valid JSON");
    assert_eq!(value["size_bytes"], 7);
    assert_eq!(value["file_type"], "file");
}

#[test]
fn json_error_goes_to_stderr_and_keeps_stdout_parseable() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run(&["--json", "file", "hash", "does-not-exist"]);
    assert_eq!(code(&out), 1);
    assert!(stdout_of(&out).trim().is_empty(), "stdout must stay clean");
    let err: serde_json::Value = serde_json::from_str(stderr_of(&out).trim())
        .expect("the JSON error envelope must be valid JSON");
    assert_eq!(err["exit_code"], 1);
    assert!(err["error"].as_str().unwrap().contains("does-not-exist"));
}

#[test]
fn plain_output_has_no_control_characters() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("x.tmp"), b"x").unwrap();

    let out = sb.run(&["--plain", "file", "clean", ".", "--pattern", "*.tmp"]);
    assert_eq!(code(&out), 0);
    let combined = format!("{}{}", stdout_of(&out), stderr_of(&out));
    assert!(
        !combined.contains('\x1b'),
        "--plain must not emit ANSI: {combined:?}"
    );
}

#[test]
fn piped_pretty_output_is_not_colourised() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("a.txt"), b"x").unwrap();

    // stdout is a pipe in tests, so `auto` colour must disable styling.
    let out = sb.run(&["file", "find", "--path", "."]);
    assert_eq!(code(&out), 0);
    assert!(
        !stdout_of(&out).contains('\x1b'),
        "piped output must not contain escape sequences"
    );
}

#[test]
fn quiet_suppresses_summary_but_keeps_payload() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("a.txt"), b"x").unwrap();

    let out = sb.run(&["--quiet", "file", "find", "--path", ".", "--count"]);
    assert_eq!(code(&out), 0);
    assert_eq!(stdout_of(&out).trim(), "1");
    assert!(
        stderr_of(&out).trim().is_empty(),
        "quiet must silence summaries"
    );
}

// ── file ──────────────────────────────────────────────────────────────────────

#[test]
fn clean_pattern_removes_only_matching_files() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("a.tmp"), b"junk").unwrap();
    fs::write(sb.work().join("b.tmp"), b"junk").unwrap();
    fs::write(sb.work().join("keep.txt"), b"keep").unwrap();

    let out = sb.run(&["file", "clean", ".", "--pattern", "*.tmp"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    assert!(!sb.work().join("a.tmp").exists());
    assert!(!sb.work().join("b.tmp").exists());
    assert!(sb.work().join("keep.txt").exists());
}

#[test]
fn clean_without_filters_fails_instead_of_silently_doing_nothing() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run(&["file", "clean", "."]);
    assert_eq!(code(&out), 1);
    assert!(stderr_of(&out).contains("Nothing to clean"));
}

#[test]
fn clean_dry_run_deletes_nothing() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("a.tmp"), b"junk").unwrap();

    let out = sb.run(&["file", "clean", ".", "--pattern", "*.tmp", "--dry-run"]);
    assert_eq!(code(&out), 0);
    assert!(sb.work().join("a.tmp").exists(), "dry run must not delete");
}

#[test]
fn global_dry_run_prevents_copy() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("src.txt"), b"payload").unwrap();

    let out = sb.run(&["--dry-run", "file", "copy", "src.txt", "dst.txt"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    assert!(
        !sb.work().join("dst.txt").exists(),
        "a global --dry-run must not write anything"
    );
}

#[test]
fn global_dry_run_prevents_sync() {
    let sb = Sandbox::new();
    sb.prepare();
    let src = sb.work().join("src");
    let dst = sb.work().join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    fs::write(src.join("f.txt"), b"x").unwrap();

    let out = sb.run(&[
        "--dry-run",
        "file",
        "sync",
        src.to_str().unwrap(),
        dst.to_str().unwrap(),
    ]);
    assert_eq!(code(&out), 0);
    assert!(!dst.join("f.txt").exists(), "dry-run sync must not copy");
}

#[test]
fn compare_reports_difference_with_exit_code_1() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("a.txt"), b"AAAA").unwrap();
    fs::write(sb.work().join("b.txt"), b"AAAB").unwrap();

    let out = sb.run(&["file", "compare", "a.txt", "b.txt"]);
    assert_eq!(code(&out), 1, "differing files must exit non-zero");

    let same = sb.run(&["file", "compare", "a.txt", "a.txt"]);
    assert_eq!(code(&same), 0);
}

#[test]
fn duplicate_min_size_filters_and_delete_requires_confirmation() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("small1.txt"), b"same").unwrap();
    fs::write(sb.work().join("small2.txt"), b"same").unwrap();
    fs::write(sb.work().join("big1.bin"), vec![3u8; 4096]).unwrap();
    fs::write(sb.work().join("big2.bin"), vec![3u8; 4096]).unwrap();

    let out = sb.run(&["--json", "file", "duplicate", ".", "--min-size", "1K"]);
    assert_eq!(code(&out), 0);
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&out)).unwrap();
    assert_eq!(value["groups"].as_array().unwrap().len(), 1);
    assert_eq!(value["groups"][0]["size_bytes"], 4096);

    // Destructive mode without --yes must refuse.
    let refused = sb.run(&["file", "duplicate", ".", "--delete-dupes"]);
    assert_eq!(code(&refused), 1);
    assert!(stderr_of(&refused).contains("--yes"));
    assert!(sb.work().join("big1.bin").exists());

    // Explicit confirmation deletes.
    let deleted = sb.run(&["file", "duplicate", ".", "--delete-dupes", "--yes"]);
    assert_eq!(code(&deleted), 0, "stderr: {}", stderr_of(&deleted));
    let remaining = fs::read_dir(sb.work())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".bin"))
        .count();
    assert_eq!(remaining, 1, "exactly one file per group must survive");
}

#[test]
fn duplicate_min_size_rejects_the_smaller_than_form() {
    let sb = Sandbox::new();
    sb.prepare();
    // `-1K` looks like a flag to the parser, so it is written in `=` form to
    // reach our own validation instead of clap's.
    let out = sb.run(&["file", "duplicate", ".", "--min-size=-1K"]);
    assert_eq!(code(&out), 1);
    assert!(stderr_of(&out).contains("lower bound"));
}

#[test]
fn decrypt_refuses_to_overwrite_its_input() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("secret"), b"ciphertext").unwrap();
    fs::write(sb.work().join("id.key"), b"AGE-SECRET-KEY-1...").unwrap();

    // No ".age" suffix and no --output → refuse rather than clobber the input.
    let out = sb.run(&["file", "decrypt", "secret", "--identity", "id.key"]);
    assert_eq!(code(&out), 1);
    assert!(
        stderr_of(&out).contains("does not end in .age"),
        "stderr: {}",
        stderr_of(&out)
    );
    assert_eq!(fs::read(sb.work().join("secret")).unwrap(), b"ciphertext");
}

#[test]
fn encrypt_rejects_a_non_regular_file() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::create_dir(sb.work().join("adir")).unwrap();
    let out = sb.run(&[
        "file",
        "encrypt",
        "adir",
        "--recipient",
        "age1ql3z7hjy54pw3hjymouwyfx24i98aw2aqx77k5",
    ]);
    assert_eq!(code(&out), 1);
    assert!(stderr_of(&out).contains("Not a regular file"));
}

#[test]
fn find_type_and_size_filters_are_validated() {
    let sb = Sandbox::new();
    sb.prepare();
    let bad_type = sb.run(&["file", "find", "--type", "bogus"]);
    assert_eq!(code(&bad_type), 1);
    assert!(stderr_of(&bad_type).contains("Unknown type filter"));

    let bad_size = sb.run(&["file", "find", "--size", "XXL"]);
    assert_eq!(code(&bad_size), 1);
    assert!(stderr_of(&bad_size).contains("Invalid size filter"));
}

#[test]
fn find_honours_config_exclusions() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::create_dir_all(sb.work().join("node_modules")).unwrap();
    fs::write(sb.work().join("node_modules/dep.js"), b"//").unwrap();
    fs::write(sb.work().join("app.js"), b"//").unwrap();

    let cfg = sb.write_config("[search]\nexclude = [\"node_modules\"]\n");
    let out = sb.run(&[
        "--config",
        cfg.to_str().unwrap(),
        "file",
        "find",
        "--path",
        ".",
        "--type",
        "f",
    ]);
    assert_eq!(code(&out), 0);
    let text = stdout_of(&out);
    assert!(text.contains("app.js"));
    assert!(
        !text.contains("node_modules"),
        "excluded paths must not appear: {text}"
    );
}

// ── dev ───────────────────────────────────────────────────────────────────────

#[test]
fn dev_hash_matches_known_vectors() {
    let sb = Sandbox::new();
    sb.prepare();

    let sha = sb.run(&["dev", "hash", "abc", "--algo", "sha256"]);
    assert_eq!(code(&sha), 0);
    assert!(stdout_of(&sha)
        .contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));

    let md5 = sb.run(&["dev", "hash", "abc", "--algo", "md5"]);
    assert_eq!(code(&md5), 0);
    assert!(stdout_of(&md5).contains("900150983cd24fb0d6963f7d28e17f72"));

    let blake3 = sb.run(&["dev", "hash", "abc", "--algo", "blake3"]);
    assert_eq!(code(&blake3), 0);
    assert!(stdout_of(&blake3)
        .contains("6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"));

    let unknown = sb.run(&["dev", "hash", "abc", "--algo", "nope"]);
    assert_eq!(code(&unknown), 1);
    assert!(stderr_of(&unknown).contains("Unknown hash algorithm"));
}

#[test]
fn dev_reads_stdin_when_no_argument_is_given() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run_with_stdin(&["dev", "hash", "--algo", "sha256"], Some("abc\n"));
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    assert!(stdout_of(&out)
        .contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
}

#[test]
fn dev_uuid_count_is_bounded() {
    let sb = Sandbox::new();
    sb.prepare();
    for bad in ["0", "101"] {
        let out = sb.run(&["dev", "uuid", "--count", bad]);
        assert_eq!(code(&out), 1, "--count {bad} must be rejected");
    }
    let ok = sb.run(&["dev", "uuid", "--count", "3"]);
    assert_eq!(code(&ok), 0);
    assert_eq!(stdout_of(&ok).lines().count(), 3);
}

#[test]
fn dev_json_invalid_input_fails() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run_with_stdin(&["dev", "json"], Some("{not json"));
    assert_eq!(code(&out), 1, "invalid JSON must not exit 0");
}

#[test]
fn dev_base64_roundtrip() {
    let sb = Sandbox::new();
    sb.prepare();
    let enc = sb.run(&["dev", "base64", "hello"]);
    assert_eq!(code(&enc), 0);
    assert_eq!(stdout_of(&enc).trim(), "aGVsbG8=");

    let dec = sb.run(&["dev", "base64", "--decode", "aGVsbG8="]);
    assert_eq!(code(&dec), 0);
    assert_eq!(stdout_of(&dec).trim(), "hello");
}

// ── search / archive / convert ────────────────────────────────────────────────

#[test]
fn search_shorthand_reports_missing_index_clearly() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run(&["search", "anything"]);
    assert_eq!(code(&out), 1);
    assert!(
        stderr_of(&out).contains("Index not found"),
        "stderr: {}",
        stderr_of(&out)
    );
}

#[test]
fn search_index_then_query_roundtrip() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("notes.txt"), b"the quick brown fox").unwrap();

    let idx = sb.run(&["search", "index", "."]);
    assert_eq!(code(&idx), 0, "stderr: {}", stderr_of(&idx));

    let q = sb.run(&["--json", "search", "quick brown"]);
    assert_eq!(code(&q), 0);
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&q)).unwrap();
    assert!(
        value
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["path"].as_str().unwrap().contains("notes.txt")),
        "expected notes.txt in results: {value}"
    );

    let info = sb.run(&["--json", "search", "info"]);
    assert_eq!(code(&info), 0);
    let info_value: serde_json::Value = serde_json::from_str(&stdout_of(&info)).unwrap();
    assert_eq!(info_value["status"], "ready");
}

#[test]
fn search_unknown_in_filter_is_rejected() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("a.txt"), b"text").unwrap();
    let _ = sb.run(&["search", "index", "."]);

    let out = sb.run(&["search", "query", "text", "--in", "nonsense"]);
    assert_eq!(code(&out), 1);
    assert!(stderr_of(&out).contains("Unknown --in value"));
}

#[test]
fn search_limit_zero_is_rejected() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("a.txt"), b"text").unwrap();
    let _ = sb.run(&["search", "index", "."]);

    let out = sb.run(&["search", "query", "text", "--limit", "0"]);
    assert_eq!(code(&out), 1);
    assert!(stderr_of(&out).contains("--limit"));
}

#[test]
fn archive_create_preserves_directory_layout() {
    let sb = Sandbox::new();
    sb.prepare();
    let proj = sb.work().join("proj");
    fs::create_dir_all(proj.join("src")).unwrap();
    fs::write(proj.join("src/main.rs"), b"fn main() {}").unwrap();

    let create = sb.run(&["archive", "create", "bundle.zip", "proj"]);
    assert_eq!(code(&create), 0, "stderr: {}", stderr_of(&create));

    let list = sb.run(&["--json", "archive", "list", "bundle.zip"]);
    assert_eq!(code(&list), 0);
    let entries: serde_json::Value = serde_json::from_str(&stdout_of(&list)).unwrap();
    let names: Vec<String> = entries
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        names.iter().any(|n| n == "proj/src/main.rs"),
        "directory prefix must survive archiving, got {names:?}"
    );

    let extract = sb.run(&["archive", "extract", "bundle.zip", "--to", "out"]);
    assert_eq!(code(&extract), 0);
    assert!(sb.work().join("out/proj/src/main.rs").exists());
}

#[test]
fn archive_create_rejects_a_missing_input() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run(&["archive", "create", "bundle.zip", "nope"]);
    assert_eq!(code(&out), 1);
    assert!(stderr_of(&out).contains("does not exist"));
    assert!(!sb.work().join("bundle.zip").exists());
}

#[test]
fn archive_extract_rejects_a_non_archive() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("junk.zip"), b"definitely not a zip").unwrap();
    let out = sb.run(&["archive", "extract", "junk.zip"]);
    assert_eq!(code(&out), 1);
}

#[test]
fn convert_roundtrip_json_to_yaml() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(
        sb.work().join("data.json"),
        br#"{"name":"omnicli","ok":true}"#,
    )
    .unwrap();

    let out = sb.run(&["convert", "run", "data.json", "data.yaml"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    let yaml = fs::read_to_string(sb.work().join("data.yaml")).unwrap();
    assert!(yaml.contains("omnicli"));

    let list = sb.run(&["--json", "convert", "list"]);
    assert_eq!(code(&list), 0);
    let pairs: serde_json::Value = serde_json::from_str(&stdout_of(&list)).unwrap();
    assert!(!pairs.as_array().unwrap().is_empty());
}

#[test]
fn convert_missing_input_fails_cleanly() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run(&["convert", "run", "missing.csv", "out.json"]);
    assert_eq!(code(&out), 1);
    assert!(stderr_of(&out).contains("not a regular file"));
}

// ── backup ────────────────────────────────────────────────────────────────────

#[test]
fn backup_create_verify_restore_cycle() {
    let sb = Sandbox::new();
    sb.prepare();
    let src = sb.work().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.txt"), b"backup me").unwrap();

    let create = sb.run(&[
        "--json",
        "backup",
        "create",
        src.to_str().unwrap(),
        "--dest",
        "bak",
        "--name",
        "nightly",
    ]);
    assert_eq!(code(&create), 0, "stderr: {}", stderr_of(&create));
    let created: serde_json::Value = serde_json::from_str(&stdout_of(&create)).unwrap();
    let snapshot_id = created["snapshot_id"].as_str().unwrap().to_owned();
    assert_eq!(
        created["verified"]["passed"], true,
        "verify_after_create defaults to true and must pass on a fresh snapshot"
    );

    let verify = sb.run(&["backup", "verify", "bak", &snapshot_id]);
    assert_eq!(code(&verify), 0, "stderr: {}", stderr_of(&verify));

    let restore = sb.run(&["backup", "restore", "bak", &snapshot_id, "--to", "restored"]);
    assert_eq!(code(&restore), 0, "stderr: {}", stderr_of(&restore));
    assert_eq!(
        fs::read(sb.work().join("restored/a.txt")).unwrap(),
        b"backup me"
    );

    let list = sb.run(&["backup", "list", "bak"]);
    assert_eq!(code(&list), 0);
    assert!(stdout_of(&list).contains(&snapshot_id));
}

#[test]
fn backup_create_dry_run_writes_nothing() {
    let sb = Sandbox::new();
    sb.prepare();
    let src = sb.work().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("a.txt"), b"x").unwrap();

    let out = sb.run(&[
        "--dry-run",
        "backup",
        "create",
        src.to_str().unwrap(),
        "--dest",
        "bak",
    ]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    assert!(
        !sb.work().join("bak").exists(),
        "dry run must not create a snapshot"
    );
}

#[test]
fn backup_verify_missing_snapshot_fails() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::create_dir_all(sb.work().join("bak")).unwrap();
    let out = sb.run(&["backup", "verify", "bak", "no-such-snapshot"]);
    assert_eq!(code(&out), 1);
}

// ── workspace ─────────────────────────────────────────────────────────────────

#[test]
fn workspace_note_todo_snippet_lifecycle() {
    let sb = Sandbox::new();
    sb.prepare();

    // Notes
    let created = sb.run(&["workspace", "note", "new", "First note", "--body", "hello"]);
    assert_eq!(code(&created), 0, "stderr: {}", stderr_of(&created));
    let listed = sb.run(&["--json", "workspace", "note", "list"]);
    let notes: serde_json::Value = serde_json::from_str(&stdout_of(&listed)).unwrap();
    assert_eq!(notes.as_array().unwrap().len(), 1);
    assert_eq!(notes[0]["title"], "First note");

    let shown = sb.run(&["workspace", "note", "show", "1"]);
    assert_eq!(code(&shown), 0);
    assert!(stdout_of(&shown).contains("hello"));

    assert_eq!(code(&sb.run(&["workspace", "note", "delete", "1"])), 0);
    let empty = sb.run(&["--json", "workspace", "note", "list"]);
    assert_eq!(stdout_of(&empty).trim(), "[]");

    // Todos
    assert_eq!(code(&sb.run(&["workspace", "todo", "add", "ship it"])), 0);
    let toggled = sb.run(&["--json", "workspace", "todo", "toggle", "1"]);
    assert_eq!(code(&toggled), 0);
    let todo: serde_json::Value = serde_json::from_str(&stdout_of(&toggled)).unwrap();
    assert_eq!(todo["done"], true);

    // Snippets
    let saved = sb.run(&[
        "workspace",
        "snippet",
        "save",
        "hello",
        "--lang",
        "rust",
        "fn main() {}",
    ]);
    assert_eq!(code(&saved), 0, "stderr: {}", stderr_of(&saved));
    let snippet = sb.run(&["--json", "workspace", "snippet", "show", "1"]);
    assert_eq!(code(&snippet), 0);
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&snippet)).unwrap();
    assert_eq!(value["language"], "rust");

    // Stats
    let stats = sb.run(&["--json", "workspace", "stats"]);
    assert_eq!(code(&stats), 0);
    let stats_value: serde_json::Value = serde_json::from_str(&stdout_of(&stats)).unwrap();
    assert_eq!(stats_value["notes"], 0);
    assert_eq!(stats_value["todos"], 1);
    assert_eq!(stats_value["snippets"], 1);
    // The path must be the store actually in use, so other processes (the
    // dashboard) can open the same file instead of guessing a default.
    assert_eq!(
        stats_value["db_path"].as_str().unwrap(),
        sb.home()
            .join(".local/share/omni/workspace.db")
            .to_string_lossy()
            .as_ref()
    );
}

#[test]
fn workspace_stats_reports_a_configured_database_path() {
    let sb = Sandbox::new();
    sb.prepare();
    let custom = sb.work().join("configured-workspace.db");
    let cfg = sb.write_config(&format!(
        "[workspace]\ndb_path = \"{}\"\n",
        custom.display()
    ));

    let out = sb.run(&[
        "--config",
        cfg.to_str().unwrap(),
        "--json",
        "workspace",
        "stats",
    ]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&out)).unwrap();
    assert_eq!(value["db_path"].as_str().unwrap(), custom.to_string_lossy());
    assert!(
        custom.exists(),
        "opening the configured path must create it"
    );
}

#[test]
fn workspace_rejects_empty_titles() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run(&["workspace", "note", "new", "   "]);
    assert_eq!(code(&out), 1);
    assert!(stderr_of(&out).contains("must not be empty"));
}

#[test]
fn workspace_uses_the_configured_database_path() {
    let sb = Sandbox::new();
    sb.prepare();
    let custom = sb.work().join("custom-workspace.db");
    let cfg = sb.write_config(&format!(
        "[workspace]\ndb_path = \"{}\"\n",
        custom.display()
    ));

    let out = sb.run(&[
        "--config",
        cfg.to_str().unwrap(),
        "workspace",
        "note",
        "new",
        "Configured",
    ]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    assert!(custom.exists(), "the configured db_path must be used");
}

// ── config subcommands ────────────────────────────────────────────────────────

#[test]
fn config_show_and_path_are_machine_readable() {
    let sb = Sandbox::new();
    sb.prepare();

    let show = sb.run(&["--json", "config", "show"]);
    assert_eq!(code(&show), 0);
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&show)).unwrap();
    assert_eq!(value["core"]["color"], "auto");
    assert!(value["search"]["exclude"].is_array());

    let path = sb.run(&["config", "path"]);
    assert_eq!(code(&path), 0);
    assert!(stdout_of(&path).trim().ends_with("omnicli.toml"));
}

#[test]
fn config_read_get_set_validate_roundtrip() {
    let sb = Sandbox::new();
    sb.prepare();
    let file = sb.work().join("app.json");
    fs::write(&file, br#"{"database":{"host":"localhost","port":5432}}"#).unwrap();
    let f = file.to_str().unwrap();

    let get = sb.run(&["config", "get", f, "database.host"]);
    assert_eq!(code(&get), 0);
    assert_eq!(stdout_of(&get).trim(), "localhost");

    let set = sb.run(&["config", "set", f, "database.port", "6000"]);
    assert_eq!(code(&set), 0, "stderr: {}", stderr_of(&set));
    let updated: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(updated["database"]["port"], 6000);

    let validate = sb.run(&["config", "validate", f]);
    assert_eq!(code(&validate), 0);

    let missing = sb.run(&["config", "get", f, "database.nope"]);
    assert_eq!(code(&missing), 1);
}

#[test]
fn config_validate_reports_invalid_files_with_exit_1() {
    let sb = Sandbox::new();
    sb.prepare();
    let file = sb.work().join("broken.json");
    fs::write(&file, b"{ not json").unwrap();
    let out = sb.run(&["--json", "config", "validate", file.to_str().unwrap()]);
    assert_eq!(code(&out), 1);
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&out)).unwrap();
    assert_eq!(value["valid"], false);
}

// ── robustness ────────────────────────────────────────────────────────────────

#[test]
fn urls_and_odd_paths_are_never_shell_executed() {
    let sb = Sandbox::new();
    sb.prepare();
    // A "path" full of shell metacharacters must be treated as a literal path,
    // never interpreted. Failure to find the file is the correct outcome.
    let marker = sb.work().join("pwned");
    let evil = format!("does-not-exist; touch {}", marker.display());
    let out = sb.run(&["file", "hash", &evil]);
    assert_eq!(code(&out), 1);
    assert!(!marker.exists(), "no shell interpretation may occur");
}

#[test]
fn empty_and_oversized_inputs_do_not_panic() {
    let sb = Sandbox::new();
    sb.prepare();

    // Empty query.
    let empty_q = sb.run(&["search", "query", ""]);
    assert_eq!(code(&empty_q), 1);
    assert!(stderr_of(&empty_q).contains("must not be empty"));

    // Empty file hashing still succeeds.
    fs::write(sb.work().join("empty.txt"), b"").unwrap();
    let hashed = sb.run(&["file", "hash", "empty.txt"]);
    assert_eq!(code(&hashed), 0);

    // Very long pattern must not panic (just report no matches).
    let long = "z".repeat(10_000);
    let out = sb.run(&["file", "find", &long, "--path", "."]);
    assert!(
        code(&out) == 0 || code(&out) == 1,
        "long pattern produced exit {}",
        code(&out)
    );
}

#[test]
fn symlinked_directories_do_not_loop_forever() {
    #[cfg(unix)]
    {
        let sb = Sandbox::new();
        sb.prepare();
        let dir = sb.work().join("loopdir");
        fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(&dir, dir.join("self")).unwrap();

        let out = sb.run(&[
            "file",
            "find",
            "--path",
            "loopdir",
            "--follow-symlinks",
            "--count",
        ]);
        // Must terminate (no hang) and exit cleanly.
        assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    }
    let _ = Path::new(".");
}

// ── doctor ────────────────────────────────────────────────────────────────────

#[test]
fn doctor_reports_every_check_with_a_real_status() {
    let sb = Sandbox::new();
    sb.prepare();

    let out = sb.run(&["--json", "doctor"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&out)).unwrap();

    let checks = value["checks"].as_array().expect("checks array");
    assert!(!checks.is_empty(), "doctor must run real checks");
    for check in checks {
        let status = check["status"].as_str().unwrap();
        assert!(
            ["pass", "warn", "fail", "info"].contains(&status),
            "unexpected status {status}"
        );
        assert!(!check["id"].as_str().unwrap().is_empty());
        assert!(!check["summary"].as_str().unwrap().is_empty());
    }

    // The summary must add up to the checks actually listed.
    let pass = checks.iter().filter(|c| c["status"] == "pass").count();
    assert_eq!(value["summary"]["pass"].as_u64().unwrap() as usize, pass);
    assert_eq!(value["summary"]["fail"].as_u64().unwrap(), 0);
    assert!(checks.iter().any(|c| c["id"] == "binary.version"));
}

#[test]
fn doctor_fails_when_the_workspace_database_is_corrupt() {
    let sb = Sandbox::new();
    sb.prepare();
    let db_path = sb.work().join("corrupt.db");
    fs::write(&db_path, b"this is definitely not a sqlite database").unwrap();
    let cfg = sb.write_config(&format!(
        "[workspace]\ndb_path = \"{}\"\n",
        db_path.display()
    ));

    let out = sb.run(&["--config", cfg.to_str().unwrap(), "doctor"]);
    assert_eq!(code(&out), 1, "a corrupt store must fail the command");
    assert!(
        stdout_of(&out).contains("FAIL"),
        "stdout: {}",
        stdout_of(&out)
    );
}

#[test]
fn doctor_rejects_an_invalid_config() {
    let sb = Sandbox::new();
    sb.prepare();
    let cfg = sb.write_config("[core]\noutput = \"nonsense\"\n");

    let out = sb.run(&["--config", cfg.to_str().unwrap(), "doctor"]);
    assert_eq!(code(&out), 3, "invalid config is a configuration error");
}

// ── report ────────────────────────────────────────────────────────────────────

#[test]
fn report_json_describes_real_state() {
    let sb = Sandbox::new();
    sb.prepare();
    fs::write(sb.work().join("one.txt"), "hello").unwrap();
    fs::write(sb.work().join("two.txt"), "world!!").unwrap();

    let out = sb.run(&["--json", "report", sb.work().to_str().unwrap()]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&out)).unwrap();

    assert_eq!(value["environment"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["files"]["files"].as_u64().unwrap(), 2);
    assert!(value["files"]["total_size_bytes"].as_u64().unwrap() > 0);
    let exts: Vec<&str> = value["files"]["top_extensions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["extension"].as_str().unwrap())
        .collect();
    assert!(exts.contains(&"txt"), "extensions: {exts:?}");
    // No index has been built in this sandbox: the section must say so.
    assert_eq!(value["search_index"]["available"], false);
    assert!(value["search_index"]["error"].is_string());
}

#[test]
fn report_markdown_is_markdown() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run(&[
        "report",
        sb.work().to_str().unwrap(),
        "--format",
        "markdown",
    ]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    let text = stdout_of(&out);
    assert!(text.starts_with("# OmniCLI report"));
    assert!(text.contains("## Environment"));
    assert!(text.contains("## Workspace"));
}

#[test]
fn report_rejects_an_unknown_format() {
    let sb = Sandbox::new();
    sb.prepare();
    let out = sb.run(&["report", "--format", "pdf"]);
    assert_eq!(code(&out), 1);
    assert!(stderr_of(&out).contains("Unknown report format"));
}

// ── security ──────────────────────────────────────────────────────────────────

#[test]
fn security_permissions_flags_world_writable_files() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let sb = Sandbox::new();
        sb.prepare();
        let dir = sb.work().join("perms");
        fs::create_dir_all(&dir).unwrap();

        // A clean tree passes.
        fs::write(dir.join("ok.txt"), "ok").unwrap();
        let clean = sb.run(&["security", "permissions", dir.to_str().unwrap()]);
        assert_eq!(code(&clean), 0, "stderr: {}", stderr_of(&clean));

        // A world-writable file is a high-severity finding.
        let exposed = dir.join("exposed.txt");
        fs::write(&exposed, "secrets").unwrap();
        fs::set_permissions(&exposed, fs::Permissions::from_mode(0o777)).unwrap();

        let out = sb.run(&["security", "permissions", dir.to_str().unwrap()]);
        assert_eq!(code(&out), 1);
        assert!(stdout_of(&out).contains("exposed.txt"));
        assert!(stdout_of(&out).contains("world-writable-file"));
    }
}

#[test]
fn security_secrets_finds_credentials_and_never_prints_them() {
    let sb = Sandbox::new();
    sb.prepare();
    let dir = sb.work().join("scan");
    fs::create_dir_all(&dir).unwrap();
    const KEY: &str = "AKIAIOSFODNN7REALKEY1";
    fs::write(dir.join("creds.env"), format!("aws_key = \"{KEY}\"\n")).unwrap();
    fs::write(
        dir.join("id_rsa.txt"),
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEow\n-----END RSA PRIVATE KEY-----\n",
    )
    .unwrap();

    let out = sb.run(&["security", "secrets", dir.to_str().unwrap()]);
    assert_eq!(code(&out), 1);
    let text = stdout_of(&out);
    assert!(text.contains("aws-access-key-id"), "stdout: {text}");
    assert!(text.contains("private-key"));
    assert!(
        !text.contains(KEY),
        "the full secret must never be printed: {text}"
    );

    // Placeholder values are not findings.
    fs::remove_file(dir.join("creds.env")).unwrap();
    fs::remove_file(dir.join("id_rsa.txt")).unwrap();
    fs::write(dir.join("example.env"), "api_key = \"your_api_key_here\"\n").unwrap();
    let clean = sb.run(&["security", "secrets", dir.to_str().unwrap()]);
    assert_eq!(code(&clean), 0, "stdout: {}", stdout_of(&clean));
}

#[test]
fn security_config_audit_reports_paths() {
    let sb = Sandbox::new();
    sb.prepare();
    sb.write_config("[core]\ncolor = \"never\"\n");

    let out = sb.run(&["--json", "security", "config"]);
    assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&out)).unwrap();
    assert!(value["config_path"]
        .as_str()
        .unwrap()
        .ends_with("omnicli.toml"));
    assert_eq!(value["high"].as_u64().unwrap(), 0);
}

#[test]
fn reindexing_twice_keeps_search_results_unique() {
    let sb = Sandbox::new();
    sb.prepare();
    let corpus = sb.work().join("corpus");
    fs::create_dir_all(&corpus).unwrap();
    for i in 0..3 {
        fs::write(
            corpus.join(format!("f{i}.txt")),
            format!("needle in file {i}"),
        )
        .unwrap();
    }

    for _ in 0..3 {
        let out = sb.run(&["search", "index", corpus.to_str().unwrap()]);
        assert_eq!(code(&out), 0, "stderr: {}", stderr_of(&out));
    }

    let query = sb.run(&["--json", "search", "needle"]);
    assert_eq!(code(&query), 0, "stderr: {}", stderr_of(&query));
    let value: serde_json::Value = serde_json::from_str(&stdout_of(&query)).unwrap();
    let paths: Vec<&str> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["path"].as_str().unwrap())
        .collect();
    let unique: std::collections::HashSet<&&str> = paths.iter().collect();
    assert_eq!(
        paths.len(),
        unique.len(),
        "the same file must not appear twice: {paths:?}"
    );
    assert_eq!(paths.len(), 3, "three files were indexed: {paths:?}");
}
