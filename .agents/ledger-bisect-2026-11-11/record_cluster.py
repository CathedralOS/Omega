#!/usr/bin/env python3
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

TAG = sys.argv[1]
LOG = Path(sys.argv[2])
MODE = sys.argv[3] if len(sys.argv) > 3 else "bisected"
REPO = Path("/agent/repos/omega")
MD = REPO / ".agents/ledger-bisect-2026-11-11/part-30.md"
log = LOG.read_text()

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

if MODE == "never-good":
    add_m = re.search(r"^ADD=([0-9a-f]+)$", log, re.M)
    subj_m = re.search(r"^SUBJ=(.+)$", log, re.M)
    pr_m = re.search(r"^PR=(\S+)$", log, re.M)
    add = add_m.group(1) if add_m else "unknown"
    pr = pr_m.group(1) if pr_m else "unknown"
    subj = subj_m.group(1).strip() if subj_m else ""
    mechanism = f"never-good since fixture introduction ({subj[:80]})"
    break_col = f"never-good since `{add[:12]}`"
    status = f"never-good (add PR {pr})"
else:
    break_m = re.search(r"^BREAK=([0-9a-f]+)$", log, re.M)
    if not break_m:
        raise SystemExit("no BREAK in log")
    sha = break_m.group(1)
    subj_m = re.search(r"^SUBJ=(.+)$", log, re.M)
    pr_m = re.search(r"^PR=(\S+)$", log, re.M)
    subj = subj_m.group(1).strip() if subj_m else ""
    pr = pr_m.group(1) if pr_m else "unknown"
    diff = subprocess.check_output(["git", "-C", str(REPO), "show", "--stat", "--format=", sha], text=True)
    touched = [l.split("|")[0].strip() for l in diff.splitlines() if "|" in l and "file changed" not in l][:3]
    mechanism = (subj.split("(#")[0].strip() + ": " + ", ".join(touched))[:200]
    break_col = f"`{sha[:12]}`"
    status = "bisected"

pattern = rf"\| {rows} \| {re.escape(kind)} \| {re.escape(diag)}.*\| (?:bisecting|queued|pending|never-good).* \|"
row = f"| {rows} | {kind} | {diag} | `{witness}` | {break_col} | {pr} | {mechanism} | {status} |"
text = MD.read_text()
text = re.sub(pattern, row, text, count=1)
MD.write_text(text)
print("updated", TAG, status)
