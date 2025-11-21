use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use glob::Pattern;

use crate::utils::parse_size;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Base directory (A)
    pub base_dir: PathBuf,

    /// Target directory (B)
    pub target_dir: PathBuf,

    /// Output directory (Differences)
    pub output_dir: PathBuf,

    /// Glob patterns to ignore (can be repeated or comma separated)
    #[arg(short, long, value_delimiter = ',', num_args = 1..)]
    pub ignore: Vec<String>,

    /// Normalize EOL (CRLF/LF) before text comparison
    #[arg(short = 'E', long)]
    pub normalize_eol: bool,

    /// Max size (in bytes) for text diff per file (e.g., 5MB, 102400)
    #[arg(short = 'S', long, default_value = "5MB")]
    pub max_text_size: String,

    /// Do not write anything; only print a summary of what would be done
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug)]
pub struct Options {
    pub normalize_eol: bool,
    pub max_text_size: u64,
    pub ignore_patterns: Vec<Pattern>,
    pub dry_run: bool,
}

pub fn build_options(args: &Args) -> Result<Options> {
    let patterns = args
        .ignore
        .iter()
        .map(|s| Pattern::new(s).with_context(|| format!("Invalid glob pattern: {s}")))
        .collect::<Result<Vec<_>>>()?;

    Ok(Options {
        normalize_eol: args.normalize_eol,
        max_text_size: parse_size(&args.max_text_size),
        ignore_patterns: patterns,
        dry_run: args.dry_run,
    })
}
