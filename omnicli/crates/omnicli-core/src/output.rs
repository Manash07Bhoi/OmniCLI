use console::{style, Style, Term};
use serde::Serialize;

/// How output should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// Human-readable, styled, coloured output.
    #[default]
    Pretty,
    /// Machine-readable JSON; no ANSI codes.
    Json,
    /// Plain text, no colours or decorations.
    Plain,
}

/// Configuration carried from global flags into every command handler.
#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub mode: OutputMode,
    pub quiet: bool,
    pub verbose: bool,
    pub dry_run: bool,
    /// Whether the terminal actually supports colour (auto-detected, then overridden by flags).
    pub color_enabled: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        let term = Term::stdout();
        Self {
            mode: OutputMode::Pretty,
            quiet: false,
            verbose: false,
            dry_run: false,
            color_enabled: term.features().colors_supported(),
        }
    }
}

impl OutputConfig {
    /// Returns true if JSON mode is active (implies no ANSI).
    pub fn is_json(&self) -> bool {
        self.mode == OutputMode::Json
    }

    /// Serialise a value as JSON to stdout.
    pub fn print_json<T: Serialize>(&self, value: &T) {
        if let Ok(s) = serde_json::to_string_pretty(value) {
            println!("{s}");
        }
    }
}

// ── Colour constants (hex → console::Color) ──────────────────────────────────
fn success_style() -> Style {
    Style::new().color256(10) // bright green
}
fn error_style() -> Style {
    Style::new().color256(9) // bright red
}
fn warning_style() -> Style {
    Style::new().color256(11) // bright yellow
}
fn info_style() -> Style {
    Style::new().color256(12) // bright blue
}
fn accent_style() -> Style {
    Style::new().color256(13) // bright magenta / purple
}
fn muted_style() -> Style {
    Style::new().color256(8) // dark grey
}

// ── Public print helpers ──────────────────────────────────────────────────────

/// ✓ Success line.  Silent if `cfg.quiet`.
pub fn print_success(cfg: &OutputConfig, msg: &str) {
    if cfg.quiet || cfg.is_json() {
        return;
    }
    if cfg.color_enabled {
        eprintln!("{} {msg}", success_style().apply_to("✓"));
    } else {
        eprintln!("[OK] {msg}");
    }
}

/// ✗ Error line. Always printed (even in quiet mode).
pub fn print_error(cfg: &OutputConfig, msg: &str) {
    if cfg.is_json() {
        return;
    }
    if cfg.color_enabled {
        eprintln!("{} {msg}", error_style().apply_to("✗"));
    } else {
        eprintln!("[FAIL] {msg}");
    }
}

/// ⚠ Warning line. Silent in quiet mode.
pub fn print_warning(cfg: &OutputConfig, msg: &str) {
    if cfg.quiet || cfg.is_json() {
        return;
    }
    if cfg.color_enabled {
        eprintln!("{} {msg}", warning_style().apply_to("⚠"));
    } else {
        eprintln!("[WARN] {msg}");
    }
}

/// ℹ Info line. Silent in quiet mode.
pub fn print_info(cfg: &OutputConfig, msg: &str) {
    if cfg.quiet || cfg.is_json() {
        return;
    }
    if cfg.color_enabled {
        eprintln!("{} {msg}", info_style().apply_to("ℹ"));
    } else {
        eprintln!("[INFO] {msg}");
    }
}

/// Accent-coloured line (used for headers / module names).
pub fn print_accent(cfg: &OutputConfig, msg: &str) {
    if cfg.quiet || cfg.is_json() {
        return;
    }
    if cfg.color_enabled {
        eprintln!("{}", accent_style().apply_to(msg));
    } else {
        eprintln!("{msg}");
    }
}

/// Muted / secondary text.
pub fn print_muted(cfg: &OutputConfig, msg: &str) {
    if cfg.quiet || cfg.is_json() {
        return;
    }
    if cfg.color_enabled {
        eprintln!("{}", muted_style().apply_to(msg));
    } else {
        eprintln!("  {msg}");
    }
}

/// Verbose/debug line. Printed only when `cfg.verbose` is true.
pub fn print_verbose(cfg: &OutputConfig, msg: &str) {
    if !cfg.verbose || cfg.is_json() {
        return;
    }
    if cfg.color_enabled {
        eprintln!("{} {msg}", muted_style().apply_to("·"));
    } else {
        eprintln!("  [DEBUG] {msg}");
    }
}

/// Print the styled module header: `⬢ omni <module> · <context>`
pub fn print_header(cfg: &OutputConfig, module: &str, context: &str) {
    if cfg.quiet || cfg.is_json() {
        return;
    }
    if cfg.color_enabled {
        eprintln!(
            "{} {} · {}",
            accent_style().apply_to("⬢"),
            style(format!("omni {module}")).bold(),
            muted_style().apply_to(context),
        );
    } else {
        eprintln!("omni {module} · {context}");
    }
}

/// Print a bolded table header row.
pub fn print_table_header(cfg: &OutputConfig, columns: &[&str], widths: &[usize]) {
    if cfg.is_json() {
        return;
    }
    let row: String = columns
        .iter()
        .zip(widths.iter())
        .map(|(col, &w)| format!("{col:<w$}"))
        .collect::<Vec<_>>()
        .join("  ");
    if cfg.color_enabled {
        eprintln!("{}", style(row).bold());
    } else {
        eprintln!("{row}");
    }
}

/// Check whether `NO_COLOR` env var is set (https://no-color.org/).
pub fn no_color_env() -> bool {
    std::env::var("NO_COLOR").is_ok()
}
