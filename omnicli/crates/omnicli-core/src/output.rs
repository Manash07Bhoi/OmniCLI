use std::sync::OnceLock;

use console::{style, Style, Term};
use serde::Serialize;

use crate::config::ColorsConfig;

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

    /// Returns true if ANSI styling should be emitted on **stdout**.
    ///
    /// Styling is suppressed in `Plain` and `Json` modes even when the terminal
    /// supports colour, so `--plain`, `--json`, and piped output never contain
    /// escape sequences.
    pub fn use_styles(&self) -> bool {
        self.color_enabled && self.mode == OutputMode::Pretty
    }

    /// Returns true if ANSI styling should be emitted on **stderr**.
    ///
    /// Diagnostics follow the same mode rules as stdout, but the colour decision
    /// additionally respects whether stderr is a terminal, so
    /// `omnicli … 2>log` never writes escape sequences into the log.
    pub fn stderr_styles(&self) -> bool {
        self.mode == OutputMode::Pretty && console::colors_enabled_stderr()
    }

    /// Serialise a value as JSON to stdout.
    ///
    /// Serialisation failure cannot be reported through a `Result` here because
    /// every call site treats JSON output as infallible; instead the failure is
    /// surfaced on stderr and `null` is emitted so stdout stays parseable.
    pub fn print_json<T: Serialize>(&self, value: &T) {
        match serde_json::to_string_pretty(value) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: failed to serialise JSON output: {e}");
                println!("null");
            }
        }
    }

    /// Accent-coloured (or plain) inline text, for use inside `println!` payloads.
    pub fn accent(&self, s: &str) -> String {
        if self.use_styles() {
            accent_style().apply_to(s).to_string()
        } else {
            s.to_owned()
        }
    }

    /// Muted/secondary inline text.
    pub fn muted(&self, s: &str) -> String {
        if self.use_styles() {
            muted_style().apply_to(s).to_string()
        } else {
            s.to_owned()
        }
    }

    /// Bold inline text.
    pub fn bold(&self, s: &str) -> String {
        if self.use_styles() {
            style(s).bold().to_string()
        } else {
            s.to_owned()
        }
    }

    /// Success-coloured inline text.
    pub fn success(&self, s: &str) -> String {
        if self.use_styles() {
            success_style().apply_to(s).to_string()
        } else {
            s.to_owned()
        }
    }

    /// Error-coloured inline text.
    pub fn error(&self, s: &str) -> String {
        if self.use_styles() {
            error_style().apply_to(s).to_string()
        } else {
            s.to_owned()
        }
    }

    /// Warning-coloured inline text.
    pub fn warning(&self, s: &str) -> String {
        if self.use_styles() {
            warning_style().apply_to(s).to_string()
        } else {
            s.to_owned()
        }
    }
}

// ── Colour palette ────────────────────────────────────────────────────────────
//
// The palette is configured once at start-up from the `[colors]` config section.
// It is never re-configured during a run, so a `OnceLock` is sufficient and
// lock-free on the hot path.

#[derive(Debug, Clone)]
struct Palette {
    success: Style,
    error: Style,
    warning: Style,
    info: Style,
    accent: Style,
    muted: Style,
}

static PALETTE: OnceLock<Palette> = OnceLock::new();

/// Map a `#RRGGBB` string onto the closest ANSI 256-colour index.
///
/// Falls back to the built-in default for the slot when the value is not a
/// valid hex colour (configuration validation should have caught that first).
fn style_from_hex(hex: &str, fallback: u8) -> Style {
    match parse_hex(hex) {
        Some((r, g, b)) => Style::new().color256(rgb_to_ansi256(r, g, b)),
        None => Style::new().color256(fallback),
    }
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Standard xterm-256 colour-cube conversion.
fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    // Greyscale ramp keeps near-grey colours readable.
    if r == g && g == b {
        if r < 8 {
            return 16;
        }
        if r > 248 {
            return 231;
        }
        return 232 + ((r as u16 - 8) * 24 / 247) as u8;
    }
    let to_cube = |c: u8| -> u8 {
        if c < 48 {
            0
        } else if c < 115 {
            1
        } else {
            ((c as u16 - 35) / 40) as u8
        }
    };
    16 + 36 * to_cube(r) + 6 * to_cube(g) + to_cube(b)
}

fn palette() -> &'static Palette {
    PALETTE.get_or_init(Palette::default)
}

impl Default for Palette {
    fn default() -> Self {
        let c = ColorsConfig::default();
        Self {
            success: style_from_hex(&c.success, 10),
            error: style_from_hex(&c.error, 9),
            warning: style_from_hex(&c.warning, 11),
            info: style_from_hex(&c.info, 12),
            accent: style_from_hex(&c.accent, 13),
            muted: style_from_hex(&c.muted, 8),
        }
    }
}

/// Configure ANSI emission per stream.
///
/// Must be called once at start-up with the final colour decision: the `console`
/// crate strips escapes from any `Style` applied while the corresponding global
/// is `false`, which is what keeps redirected output byte-clean.
pub fn set_stream_colors(stdout_enabled: bool, stderr_enabled: bool) {
    console::set_colors_enabled(stdout_enabled);
    console::set_colors_enabled_stderr(stderr_enabled);
}

/// Install the user's `[colors]` palette. Intended to be called once, before any output.
pub fn set_color_palette(cfg: &ColorsConfig) {
    let p = Palette {
        success: style_from_hex(&cfg.success, 10),
        error: style_from_hex(&cfg.error, 9),
        warning: style_from_hex(&cfg.warning, 11),
        info: style_from_hex(&cfg.info, 12),
        accent: style_from_hex(&cfg.accent, 13),
        muted: style_from_hex(&cfg.muted, 8),
    };
    // Ignore a second call: the first palette wins for the process lifetime.
    let _ = PALETTE.set(p);
}

// ── Colour accessors ─────────────────────────────────────────────────────────
fn success_style() -> Style {
    palette().success.clone()
}
fn error_style() -> Style {
    palette().error.clone()
}
fn warning_style() -> Style {
    palette().warning.clone()
}
fn info_style() -> Style {
    palette().info.clone()
}
fn accent_style() -> Style {
    palette().accent.clone()
}
fn muted_style() -> Style {
    palette().muted.clone()
}

// ── Public print helpers ──────────────────────────────────────────────────────

/// ✓ Success line. Silent if `cfg.quiet`.
pub fn print_success(cfg: &OutputConfig, msg: &str) {
    if cfg.quiet || cfg.is_json() {
        return;
    }
    if cfg.stderr_styles() {
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
    if cfg.stderr_styles() {
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
    if cfg.stderr_styles() {
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
    if cfg.stderr_styles() {
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
    if cfg.stderr_styles() {
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
    if cfg.stderr_styles() {
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
    if cfg.stderr_styles() {
        eprintln!("{} {msg}", muted_style().apply_to("·"));
    } else {
        eprintln!("  [DEBUG] {msg}");
    }
}

/// Print the styled module header: `⬢ omnicli <module> · <context>`
pub fn print_header(cfg: &OutputConfig, module: &str, context: &str) {
    if cfg.quiet || cfg.is_json() {
        return;
    }
    if cfg.stderr_styles() {
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
    if cfg.stderr_styles() {
        eprintln!("{}", style(row).bold());
    } else {
        eprintln!("{row}");
    }
}

/// Emit a machine-readable error envelope on stderr.
///
/// stdout is reserved for the success payload, so in `--json` mode errors are
/// reported on stderr as a single JSON object: `{"error": "...", "exit_code": 1}`.
pub fn print_json_error(message: &str, exit_code: i32) {
    let payload = serde_json::json!({
        "error": message,
        "exit_code": exit_code,
    });
    match serde_json::to_string(&payload) {
        Ok(s) => eprintln!("{s}"),
        Err(_) => eprintln!("error: {message}"),
    }
}

/// Check whether `NO_COLOR` env var is set (https://no-color.org/).
pub fn no_color_env() -> bool {
    std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> OutputConfig {
        OutputConfig {
            mode: OutputMode::Plain,
            quiet: false,
            verbose: false,
            dry_run: false,
            color_enabled: false,
        }
    }

    #[test]
    fn test_use_styles_respects_mode() {
        let mut cfg = plain();
        assert!(!cfg.use_styles());

        // Colour enabled + Pretty → styles.
        cfg.mode = OutputMode::Pretty;
        cfg.color_enabled = true;
        assert!(cfg.use_styles());

        // JSON must never emit ANSI, even with colour enabled.
        cfg.mode = OutputMode::Json;
        assert!(!cfg.use_styles());

        // Plain must never emit ANSI, even with colour enabled.
        cfg.mode = OutputMode::Plain;
        assert!(!cfg.use_styles());
    }

    #[test]
    fn test_inline_helpers_emit_no_ansi_when_styles_off() {
        let cfg = plain();
        for styled in [
            cfg.accent("x"),
            cfg.muted("x"),
            cfg.bold("x"),
            cfg.success("x"),
            cfg.warning("x"),
        ] {
            assert_eq!(styled, "x");
            assert!(!styled.contains('\x1b'));
        }
    }

    #[test]
    fn test_styles_enabled_when_colour_allowed_and_pretty() {
        // The `console` crate additionally strokes colour off when its process
        // global says the stream is not a terminal, so this asserts the gate we
        // control; the end-to-end colour behaviour is covered by the CLI smoke
        // tests in `tests/cli.rs`.
        let cfg = OutputConfig {
            mode: OutputMode::Pretty,
            color_enabled: true,
            ..plain()
        };
        assert!(cfg.use_styles());
        assert!(cfg.accent("x").contains('x'));
    }

    #[test]
    fn test_no_color_env_empty_is_not_set() {
        // An empty NO_COLOR is not a request for no colour (no-color.org spec).
        // Guarded because tests share a process environment.
        let prev = std::env::var("NO_COLOR").ok();
        std::env::set_var("NO_COLOR", "");
        assert!(!no_color_env());
        std::env::set_var("NO_COLOR", "1");
        assert!(no_color_env());
        match prev {
            Some(v) => std::env::set_var("NO_COLOR", v),
            None => std::env::remove_var("NO_COLOR"),
        }
    }

    #[test]
    fn test_rgb_to_ansi256_known_points() {
        assert_eq!(rgb_to_ansi256(0, 0, 0), 16);
        assert_eq!(rgb_to_ansi256(255, 255, 255), 231);
        // Pure red maps into the colour cube.
        assert_eq!(rgb_to_ansi256(255, 0, 0), 196);
    }

    #[test]
    fn test_parse_hex_rejects_bad_values() {
        assert!(parse_hex("2ECC71").is_none());
        assert!(parse_hex("#2ECC7").is_none());
        assert!(parse_hex("#GGGGGG").is_none());
        assert_eq!(parse_hex("#2ECC71"), Some((0x2E, 0xCC, 0x71)));
    }
}
