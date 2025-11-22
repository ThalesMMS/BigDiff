//
// main.rs
// BigDiff-rs
//
// Runs the BigDiff CLI: validates input/output directories, supports dry runs, and orchestrates the diff engine that produces annotated diffs.
//
// Thales Matheus Mendonça Santos - November 2025
//
// CLI entrypoint that validates input/output folders and delegates to the diff engine.
use anyhow::{bail, Context, Result};
use clap::Parser;
use std::fs;

use bigdiff::cli::{build_options, Args};
use bigdiff::diff::run_bigdiff;
use bigdiff::scanner::scan_dir;

fn main() -> Result<()> {
    // Parse CLI arguments defined in `cli.rs`.
    let args = Args::parse();

    // Resolve absolute, canonical paths so comparisons are stable.
    let a_root = args.base_dir.canonicalize().context("Invalid base_dir")?;
    let b_root = args
        .target_dir
        .canonicalize()
        .context("Invalid target_dir")?;
    let out_root = args.output_dir.clone();

    // Prevent self-comparison or writing results inside either input tree.
    if a_root == b_root {
        bail!("base_dir and target_dir cannot be the same directory.");
    }
    if out_root.exists() {
        let out_abs = out_root.canonicalize()?;
        if out_abs == a_root
            || out_abs == b_root
            || out_abs.starts_with(&a_root)
            || out_abs.starts_with(&b_root)
        {
            bail!("output_dir cannot be inside base_dir/target_dir nor be equal to them.");
        }
    } else {
        // Ensure the output directory exists before writing any files.
        fs::create_dir_all(&out_root)?;
    }

    // Parse per-run options (ignore globs, normalization flags, etc.).
    let opts = build_options(&args)?;

    // Dry-run prints a summary without writing anything to disk.
    if opts.dry_run {
        println!("== DRY RUN (Rust Simulation) ==");
        let scan_a = scan_dir(&a_root, &opts.ignore_patterns);
        let scan_b = scan_dir(&b_root, &opts.ignore_patterns);

        let only_a = scan_a
            .files
            .keys()
            .filter(|k| !scan_b.files.contains_key(*k))
            .count();
        let only_b = scan_b
            .files
            .keys()
            .filter(|k| !scan_a.files.contains_key(*k))
            .count();
        let common = scan_a
            .files
            .keys()
            .filter(|k| scan_b.files.contains_key(*k))
            .count();

        println!("Files only in Base (would be deleted): {}", only_a);
        println!("Files only in Target (would be new): {}", only_b);
        println!("Common files (would be checked): {}", common);
        return Ok(());
    }

    // Execute the main comparison workflow and collect counters.
    let counters = run_bigdiff(&a_root, &b_root, &out_root, &opts)?;

    // Provide a short, human-readable recap of what happened.
    println!("== BigDiff (Rust): Summary ==");
    println!("Equal (omitted):      {}", counters.same);
    println!("New (.new):           {}", counters.new_files);
    println!("Deleted (.deleted):   {}", counters.del_files);
    println!("Modified text:        {}", counters.mod_text);
    println!("Modified binary:      {}", counters.mod_binary);
    println!("Deleted dirs:         {}", counters.del_dirs);
    println!("Output at:            {:?}", out_root);

    Ok(())
}
