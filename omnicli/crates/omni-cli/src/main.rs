mod cli;
mod dispatch;

use anyhow::Result;
use clap::Parser;
use omni_core::{
    config::load_config,
    output::{no_color_env, OutputConfig, OutputMode},
};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = cli::Cli::parse();

    // Respect NO_COLOR env var and --no-color flag.
    let color_disabled = no_color_env() || args.no_color;

    let mode = if args.json {
        OutputMode::Json
    } else {
        OutputMode::Pretty
    };

    let mut out_cfg = OutputConfig {
        mode,
        quiet: args.quiet,
        verbose: args.verbose,
        dry_run: args.dry_run,
        color_enabled: !color_disabled,
    };

    // Load config (silently falls back to defaults if missing).
    let config = load_config(args.config.as_deref()).unwrap_or_default();

    // Apply colour setting from config when not overridden by --no-color / NO_COLOR.
    if !color_disabled {
        match config.core.color.as_str() {
            "never" => out_cfg.color_enabled = false,
            "always" => out_cfg.color_enabled = true,
            _ => {} // "auto" — already handled above
        }
    }

    // Apply output mode from config when --json was not passed.
    if !args.json {
        match config.core.output.as_str() {
            "plain" => out_cfg.mode = OutputMode::Plain,
            "json" => out_cfg.mode = OutputMode::Json,
            _ => {} // "pretty" (default)
        }
    }

    dispatch::dispatch(args.command, &out_cfg, &config)
}
