pub mod cli;
pub mod comment;
pub mod diff;
pub mod scanner;
pub mod utils;

pub use cli::{build_options, Args, Options};
pub use diff::{run_bigdiff, Counters};
pub use scanner::{scan_dir, ScanResult};
