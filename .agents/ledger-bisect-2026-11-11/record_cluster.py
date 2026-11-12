#!/usr/bin/env python3
"""Update part-30.md table row from a finished bisect log."""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

TAG = sys.argv[1]
LOG = Path(sys.argv[2])
REPO = Path("/agent/repos/omega")
MD = REPO / ".agents/ledger-bisect-2026-11-11/part-30.md"

break_m = re.search(r"^BREAK=([0-9a-f]+)$", LOG.read_text(), re.M)
subj_m = re.search(r"^SUBJ=(.+)$", LOG.read_text(), re.M)
pr_m = re.search(r"^PR=(\S+)$", LOG.read_text(), re.M)
if not break_m:
    raise SystemExit("no BREAK in log")
sha = break_m.group(1)
subj = subj_m.group(1).strip() if subj_m else ""
pr = pr_m.group(1) if pr_m else "unknown"

diff = subprocess.check_output(
    ["git", "-C", str(REPO), "show", "--stat", "--format=", sha], text=True
)
lines = [l for l in diff.splitlines() if l.strip() and "|" not in l[:3]]
mechanism = f"{subj.split('(')[0].strip()}: touches {lines[0][:100]}" if lines else subj[:100]

meta = {
    "recoverable": (6, "pass/run rejected (incomplete)", "pass/facts/recoverable_replacement_window_through_const_name_exit",
                    "`main` calls `repair`; `repair` has no executable body…"),
    "opaque": (1, "pass/run rejected (incomplete)", "pass/facts/opaque_independent_inherited_result_through_local_exit",
               "`AddressBridgeProvider::classified_make` has no executable body…"),
    "traits": (1, "pass/run rejected (incomplete)", "pass/traits/runtime_qualified_owned_requirement_calls_exit",
               "`CellReading::Reading::observed` has no executable body…"),
    "gui": (1, "pass/run rejected", "pass/effects/gui_semantic_services",
            "`machine pace` omits invokes DarwinClock from published synchronous-invocation ceiling…"),
}[TAG]

rows, kind, witness, diag = meta
row = (
    f"| {rows} | {kind} | {diag} | `{witness}` | `{sha[:12]}` | {pr} | {mechanism} | bisected |"
)
text = MD.read_text()
text = re.sub(
    rf"\| {rows} \| {re.escape(kind)} \| {re.escape(diag)}.*\| (?:bisecting|queued|pending).* \|",
    row,
    text,
    count=1,
)
MD.write_text(text)
print("updated", TAG, sha[:12], pr)
