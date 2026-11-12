#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PIN = "51e8c45c793007f211d0d9baee7de16767172444"


def mechanism(commit: str) -> str:
    subj = subprocess.check_output(
        ["git", "-C", str(ROOT), "log", "-1", "--format=%s", commit], text=True
    ).strip()
    files = subprocess.check_output(
        ["git", "-C", str(ROOT), "diff", "--name-only", f"{commit}^1", commit], text=True
    ).strip().splitlines()
    code = [f for f in files if not f.endswith(".md") and f != "tests/omega/corpus_outcomes.txt"]
    hint = code[0] if code else (files[0] if files else "")
    lane = hint.split("/")[0] if hint else ""
    return f"{subj} (touches {lane or 'ledger/doc'})"


def load_clusters() -> list[dict]:
    data = json.loads(Path("/tmp/part14_clusters_meta.json").read_text())
    return data["clusters"]


def main() -> None:
    idx = int(sys.argv[1])
    bisect_path = Path(sys.argv[2])
    bis = json.loads(bisect_path.read_text())
    clusters = load_clusters()
    c = clusters[idx]
    brk = bis["breaking_commit"]
    pr = bis.get("pr") or ""
    mech = mechanism(brk)
    md = ROOT / ".agents/ledger-bisect-2026-11-11/part-14.md"
    tsv = ROOT / ".agents/ledger-bisect-2026-11-11/part-14.tsv"
    md.parent.mkdir(parents=True, exist_ok=True)
    if not md.exists():
        md.write_text(
            f"# Ledger bisect part 14 (PIN `{PIN[:12]}`)\n\n"
            "| cluster | rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |\n"
            "| ---: | ---: | --- | --- | --- | --- | ---: | --- | --- |\n"
        )
    diag_short = c["diag_sample"][:80].replace("|", "\\|")
    row = (
        f"| {idx} | {len(c['members'])} | {c['kind']} | {diag_short} | `{c['witness']}` | "
        f"`{brk}` | {pr or '—'} | {mech.replace('|', '/')} | bisected |\n"
    )
    with md.open("a") as f:
        f.write(row)
        f.write("\nMembers:\n")
        for m in c["members"]:
            f.write(f"- `{m['fixture']}` — {m['class']}\n")
        if bis.get("good"):
            f.write(f"\nBisect good: `{bis['good'][:12]}`\n")
    if not tsv.exists():
        tsv.write_text("fixture\tclass\tfirst_diagnostic\tcluster_id\n")
    with tsv.open("a") as tf:
        for m in c["members"]:
            tf.write(f"{m['fixture']}\t{m['class']}\t{m.get('diag', '')[:200]}\t{idx}\n")


if __name__ == "__main__":
    main()
