"""Violating-caller probe for drop_requires holes (fix5)."""

from __future__ import annotations

import re
import shutil
import subprocess
import tempfile
from pathlib import Path


def violating_value_from_requires(original: str) -> str | None:
    if re.search(r"!=\s*0", original):
        return "0u32"
    if re.search(r"<\s*", original):
        return "999999u32"
    return None


def callee_machine(original: str) -> str | None:
    m = re.search(r"^\s*machine\s+(\w+)\s*\([^)]*\)\s*(?:.*\n)*?\s*requires", original, re.M)
    if m:
        return m.group(1)
    m = re.search(r"^\s*machine\s+(\w+)\s*\(", original, re.M)
    return m.group(1) if m else None


def probe_subtype(
    mutant_path: Path,
    original: str,
    check_accepted: callable,
) -> str:
    """Return inferred-entry if checker accepts mutant but rejects violating probe caller."""
    callee = callee_machine(original)
    bad = violating_value_from_requires(original)
    if not callee or not bad or not mutant_path.exists():
        return "probe-not-built"
    probe_src = mutant_path.read_text(encoding="utf-8")
    probe_src += f"\nmachine __soundness_entry_probe__() {{ {callee}({bad}); }}\n"
    with tempfile.TemporaryDirectory() as tmp:
        stage = Path(tmp) / "probe"
        shutil.copytree(mutant_path.parent, stage)
        (stage / "main.omg").write_text(probe_src, encoding="utf-8")
        mutant_ok = check_accepted(mutant_path)
        probe_ok = check_accepted(stage / "main.omg")
    if mutant_ok and not probe_ok:
        return "inferred-entry"
    return "probe-not-built"
