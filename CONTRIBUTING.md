# Contributing to BigDiff

Thanks for helping improve BigDiff.

## Before you start

- Keep changes focused. This repo is a small utility, so prefer small PRs over broad refactors.
- If you change CLI behavior or file annotations, update `README.md` in the same PR.
- If a change affects both implementations, note whether you updated both Rust and Python or intentionally changed only one.
- Avoid sharing private or production files in issues, commits, or PRs. Use small synthetic samples instead.

## Repository layout

- `src/`: Rust implementation
- `bigdiff/` and `bigdiff.py`: Python implementation
- `README.md`: user-facing behavior and usage notes

## Local checks

Run the checks that match your change:

```bash
# Quick validation
python3 -m bigdiff --help

# Build verification
cargo build
```

If you changed diff behavior, also test with a tiny pair of sample folders and describe the result in the PR.

## Pull requests

1. Open an issue first for bugs, feature requests, or user-facing behavior changes when practical.
2. Create a branch with a short descriptive name.
3. Explain the motivation, the behavior change, and any tradeoffs.
4. Link the related issue when there is one and complete `.github/pull_request_template.md`, including notes for output format or annotation syntax changes, CLI argument or behavior changes, and diff algorithm or comparison logic changes with any performance or compatibility impact.
5. Keep commits and PR scope easy to review.

## Reporting bugs

Use the bug report issue form and include:

- which implementation you ran (Rust or Python)
- command used
- expected vs actual result
- a minimal reproducible sample, sanitized if needed

## Security

Please do not post security-sensitive details in public issues. Follow `SECURITY.md` instead.
