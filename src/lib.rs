//
// lib.rs
// BigDiff-rs
//
// Library entry that re-exports modules so the binary and any external users can access CLI parsing, diff logic, scanning, and utilities.
//
// Thales Matheus Mendonça Santos - November 2025
//
// Public crate interface: re-export modules used by the binary and tests.
pub mod cli;
pub mod comment;
pub mod diff;
pub mod scanner;
pub mod utils;

pub use cli::{build_options, Args, Options};
pub use diff::{run_bigdiff, Counters};
pub use scanner::{scan_dir, ScanResult};
