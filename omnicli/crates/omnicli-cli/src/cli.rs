use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// OmniCLI — one binary, one grammar, eight command groups.
#[derive(Debug, Parser)]
#[command(
    name = "omnicli",
    version,
    author,
    about = "One CLI for file ops, search, conversion, backup, archiving, and workspace management.",
    long_about = "OmniCLI — one binary, one grammar.\n\n\
Command groups: file, search, archive, convert, config, dev, backup, workspace,\n\
doctor, report, security.\n\n\
Global flags accept both `--flag` and `--flag=value` forms. `--json`, `--plain`,\n\
`--no-color` and the NO_COLOR environment variable control output formatting;\n\
`--json` never emits ANSI escape sequences.\n\n\
Exit codes:\n  0  success\n  1  runtime failure (nothing partial was left behind beyond documented dry-run behaviour)\n  2  command-line usage error\n  3  configuration file missing or invalid\n\n\
Configuration: ~/.config/omni/omnicli.toml (override with --config).",
    propagate_version = true
)]
pub struct Cli {
    /// Structured JSON output — no ANSI codes.
    #[arg(long, global = true)]
    pub json: bool,

    /// Plain text output — no colours, no decorative glyphs.
    #[arg(long, global = true)]
    pub plain: bool,

    /// Disable colour output (also honours the NO_COLOR env var).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Suppress non-error, non-payload output (headers, summaries, progress).
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Enable debug-level diagnostics to stderr.
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    /// For destructive operations: report the plan without changing anything.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Override default config path (~/.config/omni/omnicli.toml).
    #[arg(long, value_name = "PATH", global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

// ── Top-level modules ─────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// File operations: find, copy, move, compare, duplicate, clean, hash, stats, encrypt, decrypt, sync.
    File {
        #[command(subcommand)]
        cmd: FileCmd,
    },
    /// Generate shell completion scripts (bash, zsh, fish, powershell, elvish).
    ///
    /// Usage (the script is registered for `omnicli`):
    ///   omnicli completions bash > ~/.local/share/bash-completion/completions/omnicli
    ///   omnicli completions zsh  > ~/.zsh/completions/_omnicli     # fpath must contain that dir
    ///   omnicli completions fish > ~/.config/fish/completions/omnicli.fish
    ///   source <(omnicli completions bash)                          # or just for this shell
    Completions {
        /// Target shell: bash, zsh, fish, powershell, elvish.
        shell: String,
    },
    /// Universal search across indexed files, code, and logs.
    ///
    /// Shorthand: `omnicli search "query" [--limit N] [--in TYPES] [--regex]` is
    /// equivalent to `omnicli search query "query"` with the same flags.
    #[command(args_conflicts_with_subcommands = true)]
    Search {
        /// Query to run immediately (shorthand for `search query <QUERY>`).
        #[arg(value_name = "QUERY")]
        query: Option<String>,

        /// Shorthand only: limit results (default: 20).
        #[arg(long, value_name = "N", requires = "query")]
        limit: Option<usize>,

        /// Shorthand only: filter by content types: files,pdf,code,sqlite,json,logs,zip.
        #[arg(long, value_name = "TYPES", requires = "query")]
        r#in: Option<String>,

        /// Shorthand only: use regex matching instead of full-text search.
        #[arg(long, requires = "query")]
        regex: bool,

        /// Shorthand only: match case exactly (default: case-insensitive).
        #[arg(long, requires = "query")]
        case_sensitive: bool,

        #[command(subcommand)]
        cmd: Option<SearchCmd>,
    },
    /// Archive management: create, extract, list, convert.
    Archive {
        #[command(subcommand)]
        cmd: ArchiveCmd,
    },
    /// Format conversion (format inferred from extension).
    Convert {
        #[command(subcommand)]
        cmd: ConvertCmd,
    },
    /// Show and manage the active configuration.
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Developer toolkit: hash, json, base64, uuid, regex, jwt.
    Dev {
        #[command(subcommand)]
        cmd: DevCmd,
    },
    /// Incremental backup with BLAKE3 content-hash deduplication.
    Backup {
        #[command(subcommand)]
        cmd: BackupCmd,
    },
    /// Workspace: notes, todos, snippets.
    Workspace {
        #[command(subcommand)]
        cmd: WorkspaceCmd,
    },
    /// Diagnose the local environment, configuration, and data stores.
    ///
    /// Every check runs for real; a failing check makes the command exit 1.
    Doctor,
    /// Summarise environment, storage, and files in one report.
    Report {
        /// Path whose file statistics are included (default: current directory).
        #[arg(value_name = "PATH")]
        path: Option<PathBuf>,

        /// Output format: terminal (default), json, or markdown.
        #[arg(long, value_name = "FORMAT", default_value = "terminal")]
        format: String,
    },
    /// Defensive security checks for files and data you own.
    Security {
        #[command(subcommand)]
        cmd: SecurityCmd,
    },
}

// ── omnicli security ────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum SecurityCmd {
    /// Report risky permission bits: setuid/setgid and world-writable entries.
    Permissions {
        /// Directory tree to scan.
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,
    },
    /// Scan text files for credential-like strings (values are always redacted).
    Secrets {
        /// Directory tree to scan.
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,
    },
    /// Audit configuration-file and data-directory permissions.
    Config,
}

// ── omnicli file ─────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum FileCmd {
    /// Find files matching a pattern.
    Find {
        /// Name pattern: substring, or a glob such as `*.rs` (--regex for full regex).
        #[arg(value_name = "PATTERN")]
        pattern: Option<String>,

        /// Use PATTERN as a regular expression.
        #[arg(long)]
        regex: bool,

        /// Filter by entry type: f (file), d (dir), l (symlink), any (all). Default: any.
        #[arg(long, value_name = "TYPE", default_value = "any")]
        r#type: String,

        /// Long listing: show size, modification time, and type for each entry.
        #[arg(long, short = 'l')]
        long: bool,

        /// Count only — print the number of matches without listing paths.
        #[arg(long, short = 'c')]
        count: bool,

        /// Size filter, e.g. +50M (larger than), -100K (smaller than).
        #[arg(long, value_name = "SIZE")]
        size: Option<String>,

        /// Only entries modified within this duration, e.g. 7d, 2h, 30m.
        #[arg(long, value_name = "DURATION")]
        modified: Option<String>,

        /// Root path to search.
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,

        /// Maximum directory depth.
        #[arg(long, value_name = "N")]
        max_depth: Option<usize>,

        /// Follow symbolic links. Circular symlink loops are detected via inode
        /// tracking and emitted as entries with cycle_detected=true (never infinite-loop).
        #[arg(long)]
        follow_symlinks: bool,
    },
    /// Copy a file or directory to a destination.
    Copy {
        source: PathBuf,
        dest: PathBuf,

        /// Copy directories recursively.
        #[arg(long, short = 'r')]
        recursive: bool,

        /// Re-hash after copy to verify byte-identical transfer.
        #[arg(long)]
        verify: bool,
    },
    /// Move a file or directory.
    Move {
        source: PathBuf,
        dest: PathBuf,

        /// Re-hash after move to verify byte-identical transfer.
        #[arg(long)]
        verify: bool,
    },
    /// Compare two files or directories and report differences.
    Compare {
        a: PathBuf,
        b: PathBuf,

        /// Use content hash comparison instead of byte-by-byte.
        #[arg(long)]
        hash: bool,
    },
    /// Find duplicate files under a path.
    Duplicate {
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Minimum file size to consider (e.g. 1K). Zero-byte files are always ignored.
        #[arg(long, value_name = "SIZE")]
        min_size: Option<String>,

        /// Delete every duplicate except the first file in each group.
        /// Requires --yes unless --dry-run is also given.
        #[arg(long)]
        delete_dupes: bool,

        /// Confirm the destructive --delete-dupes operation non-interactively.
        #[arg(long)]
        yes: bool,
    },
    /// Remove empty directories, stale files, or files matching a glob.
    Clean {
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Remove empty directories.
        #[arg(long)]
        empty_dirs: bool,

        /// Remove files matching this glob, e.g. '*.tmp' (matched against the file name).
        #[arg(long, value_name = "GLOB")]
        pattern: Option<String>,

        /// Remove files older than this duration, e.g. 30d, 12h.
        #[arg(long, value_name = "DURATION")]
        older_than: Option<String>,

        /// Preview changes without deleting.
        #[arg(long)]
        dry_run: bool,
    },
    /// Hash a file (BLAKE3 by default).
    Hash {
        path: PathBuf,

        /// Algorithm: blake3 (default), sha256, md5.
        #[arg(long, value_name = "ALGO")]
        algo: Option<String>,
    },
    /// Encrypt a file using age (X25519 recipient key).
    Encrypt {
        path: PathBuf,

        /// Recipient public key (age1…).
        #[arg(long, value_name = "KEY", required = true)]
        recipient: String,

        /// Output path (default: <path>.age).
        #[arg(long, value_name = "OUT")]
        output: Option<PathBuf>,
    },
    /// Decrypt an age-encrypted file.
    Decrypt {
        path: PathBuf,

        /// Path to private key file.
        #[arg(long, value_name = "KEY", required = true)]
        identity: PathBuf,

        /// Output path (default: strip .age extension).
        #[arg(long, value_name = "OUT")]
        output: Option<PathBuf>,
    },
    /// Synchronise source to destination (BLAKE3-based incremental).
    Sync {
        source: PathBuf,
        dest: PathBuf,

        /// Remove files in dest not in source.
        #[arg(long)]
        delete: bool,

        /// Preview changes without copying or deleting.
        #[arg(long)]
        dry_run: bool,
    },
    /// Get file stats (size, mtime, hash).
    Stats {
        path: PathBuf,

        /// Hash algorithm to include in stats: blake3, sha256, md5.
        #[arg(long, value_name = "ALGO")]
        algo: Option<String>,
    },
}

// ── omnicli search ───────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum SearchCmd {
    /// Query the search index.
    #[command(name = "query", alias = "q")]
    Query {
        /// Search query (matches as a phrase).
        query: String,

        /// Limit results (default: 20).
        #[arg(long, value_name = "N", default_value = "20")]
        limit: usize,

        /// Filter by content types: files,pdf,code,sqlite,json,logs,zip.
        #[arg(long, value_name = "TYPES")]
        r#in: Option<String>,

        /// Use regex matching instead of full-text search.
        #[arg(long)]
        regex: bool,

        /// Match case exactly (default: case-insensitive).
        #[arg(long)]
        case_sensitive: bool,
    },
    /// Index a directory (add or update entries).
    ///
    /// When PATH is omitted, the `search.index_paths` values from the config file
    /// are used.
    Index {
        /// One or more directories to index. Defaults to `search.index_paths`.
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,

        /// Force full re-index, removing stale entries first.
        #[arg(long)]
        rebuild: bool,

        /// Extra exclusion substrings (repeatable). Config `search.exclude` always applies.
        #[arg(long, value_name = "SUBSTRING")]
        exclude: Vec<String>,
    },
    /// Show index statistics.
    Info,
}

// ── omnicli archive ──────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum ArchiveCmd {
    /// Create an archive (format from extension).
    Create {
        /// Output archive file (e.g. backup.tar.gz).
        output: PathBuf,

        /// Files or directories to include.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },
    /// Extract an archive (format detected from magic bytes).
    Extract {
        archive: PathBuf,

        /// Destination directory (default: strip extension).
        #[arg(long, value_name = "DIR")]
        to: Option<PathBuf>,
    },
    /// List contents without extracting.
    List { archive: PathBuf },
    /// Re-package one archive format into another.
    Convert { input: PathBuf, output: PathBuf },
}

// ── omnicli convert ──────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum ConvertCmd {
    /// Convert a file (format inferred from extension).
    Run { input: PathBuf, output: PathBuf },
    /// List all supported conversion pairs.
    List,
}

// ── omnicli config ───────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print the current active omnicli configuration (defaults + config file).
    Show,
    /// Print the path where omnicli looks for its config file.
    Path,
    /// Read and display a config file (JSON, YAML, TOML, XML, INI).
    Read {
        /// Config file to read.
        path: PathBuf,
    },
    /// Get a specific key from a config file using dotted path (e.g. database.host).
    Get { path: PathBuf, key: String },
    /// Set a key in a config file. Value is parsed as JSON, falling back to string.
    Set {
        path: PathBuf,
        key: String,
        value: String,
    },
    /// Validate a config file's syntax.
    Validate { path: PathBuf },
}

// ── omnicli dev ──────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum DevCmd {
    /// Compute a cryptographic hash of text input.
    Hash {
        /// Input text to hash (reads from stdin if not provided).
        input: Option<String>,

        /// Algorithm: sha256 (default), sha1, md5, blake3.
        #[arg(long, value_name = "ALGO", default_value = "sha256")]
        algo: String,
    },
    /// Pretty-print, minify, validate, or query JSON.
    Json {
        /// JSON input (reads from stdin if not provided).
        input: Option<String>,

        /// Action: pretty (default), minify, validate.
        #[arg(long, value_name = "ACTION", default_value = "pretty")]
        action: String,

        /// Dotted key path query, e.g. .user.name or .items[0].
        #[arg(long, value_name = "PATH")]
        query: Option<String>,
    },
    /// Base64 encode or decode text.
    Base64 {
        /// Input text (reads from stdin if not provided).
        input: Option<String>,

        /// Decode instead of encode.
        #[arg(long, short = 'd')]
        decode: bool,
    },
    /// Generate UUIDs.
    Uuid {
        /// Number of UUIDs to generate (1–100).
        #[arg(long, value_name = "N", default_value = "1")]
        count: usize,

        /// UUID version: v4 (default, random) or v7 (time-ordered).
        #[arg(long = "ver", value_name = "VER", default_value = "v4")]
        ver: String,
    },
    /// Test a regex pattern against text.
    Regex {
        /// Regular expression pattern.
        pattern: String,

        /// Text to test (reads from stdin if not provided).
        text: Option<String>,

        /// Regex flags: i (case-insensitive), m (multiline), s (dotall).
        #[arg(long, value_name = "FLAGS", default_value = "")]
        flags: String,
    },
    /// Decode a JWT token (no signature verification).
    Jwt {
        /// JWT token string.
        token: Option<String>,
    },
}

// ── omnicli backup ───────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum BackupCmd {
    /// Create an incremental backup snapshot.
    Create {
        /// Source directory to back up.
        source: PathBuf,

        /// Backup destination directory. Defaults to `backup.default_dest` from config.
        #[arg(long, value_name = "DIR")]
        dest: Option<PathBuf>,

        /// Job name for this backup (used in snapshot IDs).
        #[arg(long, value_name = "NAME", default_value = "default")]
        name: String,
    },
    /// Restore files from a backup snapshot.
    Restore {
        /// Backup directory containing snapshots.
        backup_dir: PathBuf,

        /// Snapshot ID to restore (see `omnicli backup list`).
        snapshot_id: String,

        /// Target directory to restore files into.
        #[arg(long, value_name = "DIR")]
        to: PathBuf,
    },
    /// Verify backup integrity (re-hash all stored objects).
    Verify {
        /// Backup directory.
        backup_dir: PathBuf,

        /// Snapshot ID to verify.
        snapshot_id: String,
    },
    /// List all snapshots in a backup directory.
    List {
        /// Backup directory.
        backup_dir: PathBuf,
    },
}

// ── omnicli workspace ────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum WorkspaceCmd {
    /// Note management.
    Note {
        #[command(subcommand)]
        cmd: NoteCmd,
    },
    /// Todo management.
    Todo {
        #[command(subcommand)]
        cmd: TodoCmd,
    },
    /// Code snippet management.
    Snippet {
        #[command(subcommand)]
        cmd: SnippetCmd,
    },
    /// Show workspace statistics.
    Stats,
}

#[derive(Debug, Subcommand)]
pub enum NoteCmd {
    /// List all notes.
    List {
        #[arg(long, value_name = "QUERY")]
        search: Option<String>,
    },
    /// Create a new note.
    New {
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        tags: Option<String>,
    },
    /// Show a note by ID.
    Show { id: i64 },
    /// Delete a note.
    Delete { id: i64 },
}

#[derive(Debug, Subcommand)]
pub enum TodoCmd {
    /// List todos.
    List {
        /// Filter by completion state: --done true or --done false.
        #[arg(long, value_name = "BOOL", value_parser = clap::value_parser!(bool))]
        done: Option<bool>,
    },
    /// Add a new todo.
    Add {
        description: String,
        /// Due date as a Unix timestamp (seconds).
        #[arg(long, value_name = "TIMESTAMP")]
        due: Option<i64>,
    },
    /// Toggle done/undone.
    Toggle { id: i64 },
    /// Delete a todo.
    Delete { id: i64 },
}

#[derive(Debug, Subcommand)]
pub enum SnippetCmd {
    /// List snippets.
    List {
        #[arg(long)]
        lang: Option<String>,
    },
    /// Save a new snippet.
    Save {
        name: String,
        #[arg(long)]
        lang: Option<String>,
        body: String,
    },
    /// Show snippet body.
    Show { id: i64 },
    /// Delete a snippet.
    Delete { id: i64 },
}
