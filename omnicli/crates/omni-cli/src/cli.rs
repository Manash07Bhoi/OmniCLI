use std::path::PathBuf;
use clap::{Parser, Subcommand};

/// OmniCLI — one binary, one grammar, twelve modules.
#[derive(Debug, Parser)]
#[command(
    name = "omni",
    version,
    author,
    about = "One CLI for file ops, search, conversion, backup, archiving, and workspace management.",
    long_about = None,
    propagate_version = true,
)]
pub struct Cli {
    /// Structured JSON output — no ANSI codes.
    #[arg(long, global = true)]
    pub json: bool,

    /// Disable colour output (also honours the NO_COLOR env var).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Suppress non-error output.
    #[arg(long, short = 'q', global = true)]
    pub quiet: bool,

    /// Enable debug-level tracing to stderr.
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    /// For destructive operations: show plan without executing.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Override default config path (~/.config/omni/omni.toml).
    #[arg(long, value_name = "PATH", global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

// ── Top-level modules ─────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// File operations: find, copy, move, compare, duplicate, clean, hash, encrypt, decrypt, compress, sync.
    File {
        #[command(subcommand)]
        cmd: FileCmd,
    },
    /// Universal search across files, code, PDFs, SQLite, logs.
    ///
    /// Shorthand: `omni search "query"` runs an immediate search.
    Search {
        #[command(subcommand)]
        cmd: SearchCmd,
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
}

// ── omni file ─────────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum FileCmd {
    /// Find files matching a pattern.
    Find {
        /// Name pattern (substring match; use --regex for full regex).
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

        /// Minimum file size to consider (e.g. 1K).
        #[arg(long, value_name = "SIZE")]
        min_size: Option<String>,
    },
    /// Remove empty directories, temp files, or matching entries.
    Clean {
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Remove empty directories.
        #[arg(long)]
        empty_dirs: bool,

        /// Remove files matching this glob.
        #[arg(long, value_name = "GLOB")]
        pattern: Option<String>,

        /// Preview changes without deleting.
        #[arg(long)]
        dry_run: bool,
    },
    /// Hash a file (BLAKE3 by default).
    Hash {
        path: PathBuf,

        /// Algorithm: blake3 (default), sha256, md5.
        #[arg(long, value_name = "ALGO", default_value = "blake3")]
        algo: String,
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
    },
    /// Get file stats (size, mtime, permissions, hash).
    Stats {
        path: PathBuf,

        /// Hash algorithm to include in stats.
        #[arg(long, value_name = "ALGO")]
        algo: Option<String>,
    },
}

// ── omni search ───────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum SearchCmd {
    /// Query the search index.
    #[command(name = "query", alias = "q")]
    Query {
        /// Search query (supports AND, OR, NOT, "phrase").
        query: String,

        /// Limit results (default: 20).
        #[arg(long, value_name = "N", default_value = "20")]
        limit: usize,

        /// Filter by content types: files,pdf,code,sqlite,json,logs,zip.
        #[arg(long, value_name = "TYPES")]
        r#in: Option<String>,

        /// Use regex matching instead of FTS5.
        #[arg(long)]
        regex: bool,
    },
    /// Index a directory (add or update entries).
    Index {
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,

        /// Force full re-index, removing stale entries first.
        #[arg(long)]
        rebuild: bool,
    },
    /// Show index statistics.
    Info,
}

// ── omni archive ──────────────────────────────────────────────────────────────

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
    List {
        archive: PathBuf,
    },
    /// Re-package one archive format into another.
    Convert {
        input: PathBuf,
        output: PathBuf,
    },
}

// ── omni convert ──────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum ConvertCmd {
    /// Convert a file (format inferred from extension).
    Run {
        input: PathBuf,
        output: PathBuf,
    },
    /// List all supported conversion pairs.
    List,
}

// ── omni config ───────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print the current active omni configuration (defaults + config file).
    Show,
    /// Print the path where omni looks for its config file.
    Path,
    /// Read and display a config file (JSON, YAML, TOML, XML, INI).
    Read {
        /// Config file to read.
        path: PathBuf,
    },
    /// Get a specific key from a config file using dotted path (e.g. database.host).
    Get {
        path: PathBuf,
        key: String,
    },
    /// Set a key in a config file. Value is parsed as JSON, falling back to string.
    Set {
        path: PathBuf,
        key: String,
        value: String,
    },
    /// Validate a config file's syntax.
    Validate {
        path: PathBuf,
    },
}

// ── omni dev ──────────────────────────────────────────────────────────────────

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

// ── omni backup ───────────────────────────────────────────────────────────────

#[derive(Debug, Subcommand)]
pub enum BackupCmd {
    /// Create an incremental backup snapshot.
    Create {
        /// Source directory to back up.
        source: PathBuf,

        /// Backup destination directory.
        dest: PathBuf,

        /// Job name for this backup (used in snapshot IDs).
        #[arg(long, value_name = "NAME", default_value = "default")]
        name: String,
    },
    /// Restore files from a backup snapshot.
    Restore {
        /// Backup directory containing snapshots.
        backup_dir: PathBuf,

        /// Snapshot ID to restore (see `omni backup list`).
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

// ── omni workspace ────────────────────────────────────────────────────────────

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
        #[arg(long)]
        done: Option<bool>,
    },
    /// Add a new todo.
    Add {
        description: String,
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
