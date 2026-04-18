#
# io_utils.py
# BigDiff
#
# Provides file utilities for size parsing, binary detection, resilient text reading, hashing, and name collision avoidance.
#
# Thales Matheus Mendonça Santos - November 2025
from __future__ import annotations

import hashlib
from pathlib import Path


def parse_size(s: str) -> int:
    """
    Convert strings like "5MB", "200k", or "10MiB" into byte counts.
    """
    s = s.strip().lower()
    # Match longer suffixes first so "mb" does not get swallowed by the plain "b" branch.
    units = [
        ("gib", 1024**3),
        ("mib", 1024**2),
        ("kib", 1024),
        ("gb", 1000**3),
        ("mb", 1000**2),
        ("kb", 1000),
        ("g", 1000**3),
        ("m", 1000**2),
        ("k", 1000),
        ("b", 1),
    ]
    for unit, multiplier in units:
        if s.endswith(unit):
            val = float(s[: -len(unit)])
            return int(val * multiplier)
    # No unit provided: interpret as raw bytes.
    return int(s)


def is_probably_binary(path: Path, sample_bytes: int = 4096) -> bool:
    """
    Simple heuristic: binary if it contains a NUL byte or cannot be decoded.
    """
    try:
        with path.open("rb") as f:
            chunk = f.read(sample_bytes)
        if b"\x00" in chunk:
            return True
        # Try a tolerant UTF-8 decode; failure implies likely binary.
        chunk.decode("utf-8")
        return False
    except Exception:
        # If we cannot read or decode, be conservative and treat it as binary.
        return True


def read_text_best_effort(path: Path, normalize_eol: bool) -> str:
    """
    Best-effort text read: try UTF-8 then Latin-1. Normalize EOL when requested.
    """
    data = None
    try:
        data = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        data = path.read_text(encoding="latin-1")
    # Optionally normalize Windows/Mac newlines to Unix for more stable diffs.
    if normalize_eol:
        data = data.replace("\r\n", "\n").replace("\r", "\n")
    return data


def file_bytes_equal(p1: Path, p2: Path) -> bool:
    """
    Compare file contents via hash (fast enough for most cases).
    """

    def _hash(p: Path) -> str:
        # Stream file content through SHA-256 to avoid loading large files into memory.
        h = hashlib.sha256()
        with p.open("rb") as f:
            for chunk in iter(lambda: f.read(65536), b""):
                h.update(chunk)
        return h.hexdigest()

    return _hash(p1) == _hash(p2)


def ensure_parent_dir(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)


def ensure_output_target_safe(out_root: Path, target: Path) -> None:
    """
    Refuse writes when any existing output path component is a symlink.

    ensure_output_target_safe checks out_root and its ancestors before deriving
    rel from target, then walks from out_root through rel to validate children.
    """
    for component in (out_root, *out_root.parents):
        if component.is_symlink():
            if component == out_root:
                raise ValueError(f"Refusing to write into symlinked output root: {out_root}")
            raise ValueError(f"Refusing to write through symlinked output path component: {component}")

    rel = target.relative_to(out_root)
    current = out_root
    for part in rel.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError(f"Refusing to write through symlinked output path component: {current}")


def avoid_collision(path: Path) -> Path:
    """
    Avoid name collisions in the output directory by appending a numeric suffix when needed.
    Example: "file.txt.modified" -> "file.txt.modified (1)"
    """
    if not path.exists():
        return path
    base = path
    n = 1
    while True:
        candidate = Path(f"{base} ({n})")
        if not candidate.exists():
            return candidate
        n += 1


def rel_parts_with_deleted_suffix(rel: Path) -> Path:
    """
    Append ".deleted" to every part of a relative path (directory names only).
    Example: "dir/sub" -> "dir.deleted/sub.deleted"
    """
    parts = list(rel.parts)
    if not parts:
        return rel
    new_parts = [p + ".deleted" for p in parts]
    return Path(*new_parts)
