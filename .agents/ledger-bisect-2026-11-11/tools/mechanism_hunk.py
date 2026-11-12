#!/usr/bin/env python3
"""Quote one diff hunk from a breaking commit that touches the diagnostic's code path."""
from __future__ import annotations

import subprocess
import sys

PREFIXES = ("omega-rust/", "source/psi/", "source/omega/", "source/library/")
SKIP_PARTS = (
    "structure-elaboration/",
    "lowered-discharge/",
    "check-tables/",
)


def main() -> int:
    sha = sys.argv[1]
    keywords = [k.lower() for k in sys.argv[2:] if k.strip()]
    diff = subprocess.check_output(
        ["git", "show", "--format=", "-U8", sha],
        text=True,
        errors="replace",
    )
    file_name = ""
    hunk: list[str] = []
    chosen: list[str] = []
    chosen_file = ""

    def consider() -> None:
        nonlocal chosen, chosen_file
        if not file_name or not hunk:
            return
        if not file_name.startswith(PREFIXES):
            return
        if any(part in file_name for part in SKIP_PARTS):
            return
        body = "".join(hunk).lower()
        if keywords and not any(k in body for k in keywords):
            return
        # Prefer a hunk that changes a matching line, not only context.
        changed = [
            ln[1:].lower()
            for ln in hunk
            if (ln.startswith("+") or ln.startswith("-")) and not ln.startswith(("+++", "---"))
        ]
        if keywords and not any(k in ln for ln in changed for k in keywords if k in ln):
            # keyword only in context: weaker, keep only if nothing better
            if chosen:
                return
        chosen = list(hunk)
        chosen_file = file_name

    for line in diff.splitlines(True):
        if line.startswith("diff --git "):
            consider()
            parts = line.strip().split(" b/", 1)
            file_name = parts[1] if len(parts) == 2 else ""
            hunk = []
            continue
        if line.startswith("@@"):
            consider()
            hunk = [line]
            continue
        if hunk:
            hunk.append(line)
    consider()
    if not chosen:
        print("mechanism not established")
        return 0
    text = f"{chosen_file}\n" + "".join(chosen[:80])
    print(text.rstrip())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
