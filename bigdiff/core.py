#
# core.py
# BigDiff
#
# Contains the diff engine: scanning results, classifying new/modified/deleted items, and writing annotated outputs.
#
# Thales Matheus Mendonça Santos - November 2025
from __future__ import annotations

import difflib
import os
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import List, Set

from .comment_styles import CommentStyle, comment_style_for
from .io_utils import (
    avoid_collision,
    ensure_output_target_safe,
    ensure_parent_dir,
    file_bytes_equal,
    is_probably_binary,
    read_text_best_effort,
    rel_parts_with_deleted_suffix,
)
from .scanner import ScanResult, scan_dir


@dataclass
class Options:
    normalize_eol: bool
    max_text_size: int
    ignore_patterns: List[str]
    dry_run: bool


@dataclass
class Counters:
    same: int = 0
    new_files: int = 0
    del_files: int = 0
    mod_text: int = 0
    mod_binary: int = 0
    del_dirs: int = 0


def annotate_text_diff(a_path: Path, b_path: Path, style: CommentStyle, normalize_eol: bool) -> str:
    """
    Return annotated content by merging lines from A (source) and B (target):
    - Lines "- " become comments "DELETED: ..."
    - Lines "+ " get a "NEW" suffix
    - Lines "  " are kept as-is
    """
    # Read both files up front, honoring the optional end-of-line normalization.
    a_text = read_text_best_effort(a_path, normalize_eol)
    b_text = read_text_best_effort(b_path, normalize_eol)

    a_lines = a_text.splitlines(keepends=True)
    b_lines = b_text.splitlines(keepends=True)

    output_lines: List[str] = []
    # difflib.ndiff tags each line with a prefix ("- ", "+ ", "  "); we convert that
    # into inline comments or markers instead of the traditional unified diff.
    for tagline in difflib.ndiff(a_lines, b_lines):
        if tagline.startswith("  "):
            output_lines.append(tagline[2:])
        elif tagline.startswith("- "):
            output_lines.append(style.deleted_line(tagline[2:]))
        elif tagline.startswith("+ "):
            output_lines.append(style.append_new_suffix(tagline[2:]))
        else:
            # Ignore ndiff hint lines that start with "? " (they are only metadata).
            continue

    return "".join(output_lines)


def copy_deleted_tree(head_rel: Path, scan_a: ScanResult, out_root: Path, counters: Counters) -> Set[Path]:
    """
    Copy an entire deleted subtree (present only in A).
    - Directories receive a ".deleted" suffix
    - Files receive a ".deleted" suffix
    Returns a set of relative file paths that were processed to avoid duplicate work in later calls.
    """
    processed_files: Set[Path] = set()
    head_abs = scan_a.roots / head_rel
    # Walk the subtree once; apply ".deleted" to every directory and file name along the way.
    for dirpath, dirnames, filenames in os.walk(head_abs, followlinks=False):
        rel_dir = Path(os.path.relpath(dirpath, scan_a.roots))
        # Destination directory mirrors the source path but with ".deleted" appended to each part.
        out_dir = out_root / rel_parts_with_deleted_suffix(rel_dir)
        ensure_output_target_safe(out_root, out_dir)
        out_dir.mkdir(parents=True, exist_ok=True)
        counters.del_dirs += 1 if rel_dir == head_rel else 0  # Count only the subtree root.

        # Copy files, tagging them with ".deleted" and avoiding name collisions in the output tree.
        for fn in filenames:
            rel_file = rel_dir / fn
            src_file = scan_a.roots / rel_file
            dst_file = out_root / rel_parts_with_deleted_suffix(rel_file)
            dst_file = dst_file.with_name(dst_file.name + ".deleted")
            ensure_output_target_safe(out_root, dst_file)
            ensure_parent_dir(dst_file)
            dst_file = avoid_collision(dst_file)
            ensure_output_target_safe(out_root, dst_file)
            shutil.copy2(src_file, dst_file)
            counters.del_files += 1
            processed_files.add(rel_file)
    return processed_files


def bigdiff(a_root: Path, b_root: Path, out_root: Path, opts: Options) -> Counters:
    # Collect a snapshot of both directory trees up front to avoid repeated I/O.
    scan_a = scan_dir(a_root, opts.ignore_patterns)
    scan_b = scan_dir(b_root, opts.ignore_patterns)

    counters = Counters()

    # Deleted directories (present in A but missing in B) are handled by their top-most heads to avoid duplicating work.
    del_dirs_all = {d for d in scan_a.dirs if d not in scan_b.dirs}
    head_del_dirs: List[Path] = []
    for d in sorted(del_dirs_all, key=lambda p: len(p.parts)):
        if not any(is_parent(x, d) for x in head_del_dirs):
            head_del_dirs.append(d)

    processed_deleted_files: Set[Path] = set()
    for head in head_del_dirs:
        processed_deleted_files |= copy_deleted_tree(head, scan_a, out_root, counters)

    # Deleted files that were not covered by a removed subtree get copied individually.
    for rel_a, abs_a in scan_a.files.items():
        if rel_a in processed_deleted_files:
            continue
        if rel_a not in scan_b.files:
            dst = out_root / rel_a
            dst = dst.with_name(dst.name + ".deleted")
            ensure_output_target_safe(out_root, dst)
            ensure_parent_dir(dst)
            dst = avoid_collision(dst)
            ensure_output_target_safe(out_root, dst)
            shutil.copy2(abs_a, dst)
            counters.del_files += 1

    # New files (present only in B) are copied with a ".new" suffix.
    for rel_b, abs_b in scan_b.files.items():
        if rel_b not in scan_a.files:
            dst = out_root / rel_b
            dst = dst.with_name(dst.name + ".new")
            ensure_output_target_safe(out_root, dst)
            ensure_parent_dir(dst)
            dst = avoid_collision(dst)
            ensure_output_target_safe(out_root, dst)
            shutil.copy2(abs_b, dst)
            counters.new_files += 1

    # For files that exist in both trees, decide whether they are identical or need annotation.
    for rel in sorted(set(scan_a.files.keys()).intersection(scan_b.files.keys())):
        a_file = scan_a.files[rel]
        b_file = scan_b.files[rel]

        try:
            same = file_bytes_equal(a_file, b_file)
        except Exception:
            # If comparison fails (e.g., permissions), err on the side of marking as modified.
            same = False

        if same:
            counters.same += 1
            continue

        # Modified file: pick a comment style based on extension and choose an output target.
        style = comment_style_for(rel)
        dst = out_root / rel
        dst = dst.with_name(dst.name + ".modified")
        ensure_output_target_safe(out_root, dst)
        ensure_parent_dir(dst)
        dst = avoid_collision(dst)
        ensure_output_target_safe(out_root, dst)

        # Binary or very large files are copied as-is instead of line-diffed to keep output manageable.
        size_b = b_file.stat().st_size
        is_bin = is_probably_binary(b_file)
        if is_bin or size_b > opts.max_text_size:
            shutil.copy2(b_file, dst)
            counters.mod_binary += 1
            # Create a side note explaining why the file was treated as binary/large.
            note = dst.with_suffix(dst.suffix + ".NOTE.txt")
            note_content = (
                f"Arquivo tratado como binário ou grande demais para diff de linhas.\n"
                f"Origem base (A): {a_file}\n"
                f"Origem alvo (B): {b_file}\n"
                f"Tamanho: {size_b} bytes\n"
                f"Estrategia: cópia direta do alvo para '.modified'.\n"
            )
            ensure_output_target_safe(out_root, note)
            note.write_text(note_content, encoding="utf-8")
        else:
            annotated = annotate_text_diff(a_file, b_file, style, opts.normalize_eol)
            dst.write_text(annotated, encoding="utf-8")
            counters.mod_text += 1

    return counters


def is_parent(parent: Path, child: Path) -> bool:
    # Returns True when "parent" is an ancestor of "child" (or equal), otherwise False.
    try:
        child.relative_to(parent)
        return True
    except Exception:
        return False
