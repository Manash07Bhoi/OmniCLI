mod cli;
mod dispatch;
mod doctor;
mod report;
mod security;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use omnicli_core::{
    config::load_config,
    output::{
        no_color_env, print_json_error, set_color_palette, set_stream_colors, OutputConfig,
        OutputMode,
    },
};
use std::io::IsTerminal;

/// Exit code used for every runtime failure.
const EXIT_FAILURE: i32 = 1;
/// Exit code used when the configuration file is missing or invalid.
const EXIT_CONFIG: i32 = 3;

/// Restore the default SIGPIPE disposition on Unix.
///
/// Rust ignores SIGPIPE, which turns `omnicli … | head -1` into an EPIPE write
/// error and prints a panic message. Every Unix tool dies quietly when its
/// reader goes away, so restore that behaviour once, at startup.
#[cfg(unix)]
fn restore_default_sigpipe() {
    // SAFETY: setting a signal disposition to SIG_DFL is async-signal-safe and
    // runs before any threads exist.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn restore_default_sigpipe() {}

/// Write the whole buffer to stdout, reporting IO errors to the caller.
fn write_stdout(bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    out.write_all(bytes)?;
    out.flush()
}

fn main() {
    restore_default_sigpipe();
    let code = match run() {
        Ok(()) => 0,
        Err(e) => {
            let code = exit_code_for(&e);
            if wants_json() {
                // stdout is reserved for the success payload, so the error
                // envelope goes to stderr as a single JSON object.
                print_json_error(&format!("{e:#}"), code);
            } else {
                eprintln!("error: {e:#}");
            }
            code
        }
    };
    if code != 0 {
        std::process::exit(code);
    }
}

/// True when `--json` was passed anywhere on the command line.
fn wants_json() -> bool {
    std::env::args().any(|a| a == "--json")
}

/// Map an error to a process exit code.
///
/// `0` success · `1` runtime failure · `2` usage error (emitted by clap) ·
/// `3` configuration error.
fn exit_code_for(err: &anyhow::Error) -> i32 {
    match err.downcast_ref::<omnicli_core::ConfigError>() {
        Some(_) => EXIT_CONFIG,
        None => EXIT_FAILURE,
    }
}

fn run() -> Result<()> {
    let cli::Cli {
        json,
        no_color,
        plain,
        quiet,
        verbose,
        dry_run,
        config: config_path,
        command,
    } = cli::Cli::parse();

    // Shell completions are handled before building the output config because they
    // write directly to stdout and must not be mixed with logging/colour output.
    if let cli::Commands::Completions { shell } = &command {
        let shell: Shell = shell.parse().map_err(|_| {
            anyhow::anyhow!(
                "Unknown shell '{shell}'. Valid values: bash, zsh, fish, powershell, elvish"
            )
        })?;
        let mut cmd = cli::Cli::command();
        // Register the completions for the name the binary actually has, so
        // `omnicli <TAB>` works after installing the generated script.
        let bin_name = cmd.get_name().to_string();
        // Buffer first: `generate` panics if the write fails, and a closed
        // stdout (`omnicli completions bash | head -1`) is not an error on
        // platforms where EPIPE is not a signal.
        let mut buf: Vec<u8> = Vec::new();
        generate(shell, &mut cmd, bin_name, &mut buf);
        if let Err(e) = write_stdout(&buf) {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(e.into());
        }
        return Ok(());
    }

    // Load and validate configuration. A malformed or invalid config is a hard
    // error: silently substituting defaults would hide the user's mistake.
    let config = load_config(config_path.as_deref())?;

    let mode = if json {
        OutputMode::Json
    } else if plain {
        OutputMode::Plain
    } else {
        match config.core.output.as_str() {
            "plain" => OutputMode::Plain,
            "json" => OutputMode::Json,
            _ => OutputMode::Pretty,
        }
    };

    // Colour decision, in precedence order:
    //   1. NO_COLOR / --no-color / --plain / --json force colour off;
    //   2. `core.color = "always" | "never"` from the config file forces on/off;
    //   3. otherwise colour is enabled only when the stream is a terminal, so
    //      piping or redirecting output never emits escape sequences.
    let colour_allowed =
        !(no_color_env() || no_color || plain || json) && mode == OutputMode::Pretty;
    let forced = match config.core.color.as_str() {
        "always" => Some(true),
        "never" => Some(false),
        _ => None,
    };

    let stdout_color = match forced {
        Some(v) => v && colour_allowed,
        None => colour_allowed && std::io::stdout().is_terminal(),
    };
    let stderr_color = match forced {
        Some(v) => v && colour_allowed,
        None => colour_allowed && std::io::stderr().is_terminal(),
    };

    // Push the decision into the styling layer: `Style` rendering strips ANSI
    // whenever the corresponding stream global is false.
    set_stream_colors(stdout_color, stderr_color);

    let out_cfg = OutputConfig {
        mode,
        quiet,
        verbose,
        dry_run,
        color_enabled: stdout_color,
    };

    // Install the configured colour palette before any output is produced.
    set_color_palette(&config.colors);

    // `--dry-run` is a global flag; individual subcommands also accept their own
    // (e.g. `file clean --dry-run`). The effective value is the logical OR.
    dispatch::dispatch(command, dry_run, &out_cfg, &config)
}
