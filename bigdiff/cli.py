#
# cli.py
# BigDiff
#
# Parses command-line arguments, validates user input paths, and orchestrates the BigDiff run or dry-run summary.
#
# Thales Matheus Mendonça Santos - November 2025
from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Optional, Sequence

from .core import Options, bigdiff, is_parent
from .io_utils import ensure_output_target_safe, parse_size, rel_parts_with_deleted_suffix
from .scanner import scan_dir


def parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    # Keep CLI strings in Portuguese to preserve the original UX, but the logic is explained in English comments below.
    p = argparse.ArgumentParser(
        prog="BigDiff",
        description="Gera uma pasta de diferenças enriquecidas entre duas árvores de diretórios.",
    )
    p.add_argument("pasta1", type=Path, help="Diretório base (A).")
    p.add_argument("pasta2", type=Path, help="Diretório alvo (B).")
    p.add_argument("pasta3", type=Path, help="Diretório de saída (diferenças). Será criado se não existir.")
    p.add_argument("--ignore", "-i", action="append", default=[], help="Padrões glob para ignorar (pode repetir ou separar por vírgula).")
    p.add_argument("--normalize-eol", "-E", action="store_true", help="Normaliza EOL (CRLF/LF) antes da comparação de texto.")
    p.add_argument("--max-text-size", "-S", type=str, default="5MB", help="Máximo (em bytes) para diff de texto por arquivo. Acima disso trata como binário. Ex.: 5MB, 8MiB.")
    p.add_argument("--dry-run", action="store_true", help="Não escreve nada; somente imprime um sumário do que faria.")
    return p.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = parse_args(argv)

    # Keep raw output path components visible until symlink and overlap checks finish.
    raw_a: Path = args.pasta1
    raw_b: Path = args.pasta2
    raw_out: Path = args.pasta3

    if not raw_a.exists() or not raw_a.is_dir():
        print(f"[ERRO] pasta1 inválida: {raw_a}", file=sys.stderr)
        return 2
    if not raw_b.exists() or not raw_b.is_dir():
        print(f"[ERRO] pasta2 inválida: {raw_b}", file=sys.stderr)
        return 2
    if raw_a == raw_b:
        print("[ERRO] pasta1 e pasta2 não podem ser o mesmo diretório.", file=sys.stderr)
        return 2

    try:
        ensure_output_target_safe(raw_out, raw_out)
    except ValueError as exc:
        print(f"[ERRO] {exc}", file=sys.stderr)
        return 2

    if raw_out == raw_a or raw_out == raw_b or is_parent(raw_a, raw_out) or is_parent(raw_b, raw_out):
        print("[ERRO] pasta3 não pode ser dentro de pasta1/pasta2, nem igual a elas.", file=sys.stderr)
        return 2

    # Canonicalize only after validating the raw path spelling so symlink components are not hidden.
    a_root = raw_a.resolve()
    b_root = raw_b.resolve()
    out_root = raw_out.resolve()

    if a_root == b_root:
        print("[ERRO] pasta1 e pasta2 não podem ser o mesmo diretório.", file=sys.stderr)
        return 2
    # Allow reusing the output directory, but forbid overlap with inputs to avoid clobbering.
    if out_root == a_root or out_root == b_root or is_parent(a_root, out_root) or is_parent(b_root, out_root):
        print("[ERRO] pasta3 não pode ser dentro de pasta1/pasta2, nem igual a elas.", file=sys.stderr)
        return 2

    if not out_root.exists():
        out_root.mkdir(parents=True, exist_ok=True)

    try:
        max_text_size = parse_size(str(args.max_text_size))
    except Exception:
        print("[ERRO] Valor inválido para --max-text-size.", file=sys.stderr)
        return 2

    opts = Options(
        normalize_eol=bool(args.normalize_eol),
        max_text_size=max_text_size,
        ignore_patterns=list(args.ignore or []),
        dry_run=bool(args.dry_run),
    )

    if opts.dry_run:
        # Dry run: scan both trees and only show a summary of planned operations.
        scan_a = scan_dir(a_root, opts.ignore_patterns)
        scan_b = scan_dir(b_root, opts.ignore_patterns)

        del_dirs_all = {d for d in scan_a.dirs if d not in scan_b.dirs}
        head_del_dirs = []
        for d in sorted(del_dirs_all, key=lambda p: len(p.parts)):
            # Track only top-level deletions so nested deletions are not reported twice.
            if not any(is_parent(x, d) for x in head_del_dirs):
                head_del_dirs.append(d)

        only_a_files = [rel for rel in scan_a.files.keys() if rel not in scan_b.files.keys()]
        only_b_files = [rel for rel in scan_b.files.keys() if rel not in scan_a.files.keys()]
        common = [rel for rel in scan_a.files.keys() if rel in scan_b.files.keys()]

        print("== DRY RUN ==")
        print(f"Pastas deletadas (top-level): {len(head_del_dirs)}")
        for d in head_del_dirs:
            print(f"  [DIR-DEL] {d} -> {rel_parts_with_deleted_suffix(d)}")
        print(f"Arquivos deletados (fora de subárvore deletada): {len(only_a_files)}")
        for r in only_a_files[:20]:
            print(f"  [DEL] {r} -> {r.name}.deleted")
        print(f"Arquivos novos: {len(only_b_files)}")
        for r in only_b_files[:20]:
            print(f"  [NEW] {r} -> {r.name}.new")
        print(f"Arquivos comuns a verificar: {len(common)}")
        # Skip content comparison during dry-run to keep the preview fast.
        return 0

    try:
        counters = bigdiff(a_root, b_root, out_root, opts)
    except ValueError as exc:
        print(f"[ERRO] {exc}", file=sys.stderr)
        return 2

    print("== BigDiff: resumo ==")
    print(f"Iguais (omitidos):    {counters.same}")
    print(f"Novos (.new):         {counters.new_files}")
    print(f"Deletados (.deleted): {counters.del_files}")
    print(f"Modificados texto:    {counters.mod_text}")
    print(f"Modificados binário:  {counters.mod_binary}")
    print(f"Pastas deletadas:     {counters.del_dirs}")
    print(f"Saída em:             {out_root}")

    return 0
