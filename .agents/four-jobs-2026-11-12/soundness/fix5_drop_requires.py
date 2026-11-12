"""soundness_fix5: modular entry; drop_requires probing."""

from __future__ import annotations

import re
import shutil
import subprocess
import tempfile
from pathlib import Path

FIX5_MACHINES_QUOTE = (
    "language/machines.md",
    "## Nested machines and identity",
    "A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.",
)

FIX5_STATE_QUOTE = (
    "language/state_contracts.md",
    "## Private entry inference",
    "An authored `requires` is complete, including on a private helper. Inference cannot silently conjoin further entry conditions to make the body check.",
)


def machine_name_before_requires(original: str, clause: str) -> str | None:
    marker = f"requires {clause}"
    idx = original.find(marker)
    if idx < 0:
        return None
    names = re.findall(r"machine\s+([\w:()<>, '\-]+)", original[:idx])
    return names[-1].strip() if names else None


def fix5_spec_bundle() -> tuple[str, str, str]:
    path, heading, q1 = FIX5_MACHINES_QUOTE
    _, _, q2 = FIX5_STATE_QUOTE
    return path, heading, f"{q1} {q2}"


def _machine_body(mutant: str, machine: str) -> str:
    short = machine.split("::")[-1].split("(")[0].strip()
    pat = re.compile(rf"machine\s+{re.escape(short)}\b[^{{]*\{{", re.MULTILINE)
    m = pat.search(mutant)
    if not m:
        return ""
    start = m.end()
    depth = 1
    i = start
    while i < len(mutant) and depth:
        if mutant[i] == "{":
            depth += 1
        elif mutant[i] == "}":
            depth -= 1
        i += 1
    return mutant[start : i - 1]


INDEXED_ACCESS_RE = re.compile(r"\[[^\]]+\]")


def _clause_in_text(clause: str, text: str) -> bool:
    norm = re.sub(r"\s+", " ", clause.strip())
    tnorm = re.sub(r"\s+", " ", text.strip())
    if norm in tnorm or tnorm in norm:
        return True
    parts = [p.strip() for p in re.split(r"&&|\|\|", norm) if p.strip()]
    return bool(parts) and all(p in tnorm for p in parts)


def _if_true_blocks(body: str) -> list[tuple[str, str, str]]:
    """(full if opener line, condition text, true-branch body text)."""
    out: list[tuple[str, str, str]] = []
    i = 0
    lines = body.splitlines(keepends=True)
    offset = 0
    for raw in lines:
        stripped = raw.strip()
        if not stripped.startswith("if "):
            offset += len(raw)
            continue
        m = re.match(r"if\s+(.+?)\s*\{", stripped)
        if not m:
            offset += len(raw)
            continue
        cond = m.group(1)
        brace = offset + raw.find("{")
        depth = 0
        j = brace
        while j < len(body):
            if body[j] == "{":
                depth += 1
            elif body[j] == "}":
                depth -= 1
                if depth == 0:
                    block = body[brace + 1 : j]
                    opener = stripped.split("{", 1)[0].strip() + " {"
                    out.append((opener, cond, block))
                    break
            j += 1
        offset += len(raw)
    return out


def body_reestablishes_clause(mutant: str, machine: str, clause: str) -> tuple[bool, str, str]:
    """
    In-machine restatement (not callers).
    Guards count only when the removed fact is in the condition and an indexed
    access sits in the guard's true branch. Returns (ok, requires_quote, restate_quote).
    """
    body = _machine_body(mutant, machine)
    if not body:
        return False, clause, ""
    for opener, cond, branch in _if_true_blocks(body):
        if not _clause_in_text(clause, cond):
            continue
        access_ln = ""
        for bln in branch.splitlines():
            if INDEXED_ACCESS_RE.search(bln):
                access_ln = bln.strip()
                break
        if access_ln:
            restate = f"{opener} ... | access: `{access_ln}`"
            return True, clause, restate
    for ln in body.splitlines():
        s = ln.strip()
        if not s or s.startswith("//") or s.startswith("transition") or s.startswith("if "):
            continue
        if _clause_in_text(clause, s):
            return True, clause, s
    return False, clause, ""


def violating_arg_for_clause(clause: str) -> str | None:
    if "==" in clause:
        left, right = [x.strip() for x in clause.split("==", 1)]
        if right.isdigit() or right.endswith("u64") or right.endswith("u32"):
            return "0u64" if "u64" in right else "0"
        return "0"
    if ".len ==" in clause:
        return "&mut backing[..]"  # unlikely generic
    return "0"


def probe_violating_caller(
    repo: Path,
    mutant_dir: Path,
    machine_short: str,
    violate_arg: str,
    omega_bin: Path,
) -> tuple[str, str]:
    """Return (subtype, detail): unchecked | inferred-entry | probe-not-built."""
    if not mutant_dir.is_dir() or not omega_bin.is_file():
        return "probe-not-built", "missing probe deps"
    main = mutant_dir / "main.omg"
    if not main.is_file():
        return "probe-not-built", "no main.omg"
    short = machine_short.split("::")[-1].split("(")[0].strip()
    probe_src = (
        main.read_text()
        + f"\nmachine __shard7_fix5_probe() -> u64 {{ {short}({violate_arg}) }}\n"
    )
    with tempfile.TemporaryDirectory(prefix="fix5probe-") as td:
        staging = Path(td)
        for item in mutant_dir.iterdir():
            if item.is_file():
                shutil.copy2(item, staging / item.name)
        (staging / "main.omg").write_text(probe_src)
        base = mutant_dir / "main.omg"
        try:
            m0 = subprocess.run(
                [str(omega_bin), "--check", str(base)],
                cwd=repo,
                capture_output=True,
                text=True,
                timeout=120,
                stdin=subprocess.DEVNULL,
            )
            m1 = subprocess.run(
                [str(omega_bin), "--check", str(staging / "main.omg")],
                cwd=repo,
                capture_output=True,
                text=True,
                timeout=120,
                stdin=subprocess.DEVNULL,
            )
        except subprocess.TimeoutExpired:
            return "probe-not-built", "probe timeout"
        if m0.returncode != 0:
            return "probe-not-built", "base mutant no longer accepts"
        if m1.returncode == 0:
            return "unchecked", f"probe {short}({violate_arg}) accepted"
        return "inferred-entry", f"probe {short}({violate_arg}) rejected (exit {m1.returncode})"
