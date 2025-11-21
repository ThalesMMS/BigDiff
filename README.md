# BigDiff-rs

A robust command-line utility written in Rust designed to compare two directory trees and generate a comprehensive report of their differences in a third directory.

Unlike standard `diff` tools that output a patch file or terminal output, **BigDiff** creates a physical copy of the changes, making it easier to visualize, review, and merge large-scale refactors or migrations.

## Features

*   **Directory Comparison:** Recursively compares a "Base" directory (A) against a "Target" directory (B).
*   **Physical Output:** Generates a result directory containing:
    *   `.new`: Files present only in Target.
    *   `.deleted`: Files present only in Base.
    *   `.modified`: Files with content differences, annotated inline.
*   **Inline Annotations:** For text files, deleted lines are preserved as comments (e.g., `// DELETED: line code`) and inserted lines are marked (e.g., `code // NEW`), respecting the comment syntax of the file's language (C-like, Python-like, HTML, etc.).
*   **Binary Handling:** Automatically detects binary files or large text files, copying the new version intact instead of attempting a line-by-line diff.
*   **Flexible Configuration:** Supports ignore patterns (glob), EOL normalization, and configurable text size limits.

## Installation

### Prerequisites
*   [Rust Toolchain](https://www.rust-lang.org/tools/install) (cargo & rustc)

### Build from Source

```bash
git clone https://github.com/yourusername/BigDiff-rs.git
cd BigDiff-rs
cargo build --release
```

The binary will be located at `target/release/bigdiff`.

## Usage

```bash
cargo run -- [OPTIONS] <base_dir> <target_dir> <output_dir>
```

### Arguments

*   `<base_dir>`: The source/original directory (A).
*   `<target_dir>`: The target/new directory (B).
*   `<output_dir>`: The directory where the difference report will be generated.

### Options

*   `--ignore <pattern>`: Glob patterns to ignore (e.g., `*.log`, `node_modules`). Can be repeated.
*   `-E, --normalize-eol`: Normalize line endings (CRLF/LF) before comparing text content.
*   `-S, --max-text-size <size>`: Maximum size for text diff per file before treating it as binary (default: "5MB").
*   `--dry-run`: Print a summary of changes without writing any files.

### Example

Compare version 1 and version 2 of a project, ignoring temporary files:

```bash
cargo run -- ./v1 ./v2 ./diff_output --ignore "*.tmp" --ignore "build/" --normalize-eol
```

## Output Structure

The `<output_dir>` will mirror the structure of your project but with modified filenames:

*   `filename.ext.new`: File exists in `target_dir` but not `base_dir`.
*   `filename.ext.deleted`: File exists in `base_dir` but not `target_dir`.
*   `filename.ext.modified`: File exists in both but content differs.
    *   Open these files to see inline `DELETED` and `NEW` annotations.
*   `filename.ext.modified.NOTE.txt`: If a file was too large or binary, this note explains why a direct copy was made instead of a text diff.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
