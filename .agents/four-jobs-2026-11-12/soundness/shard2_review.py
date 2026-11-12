#!/usr/bin/env python3
"""Lead-review helpers for soundness mutation shard confirmation."""
from __future__ import annotations

import difflib
import importlib.util
import re
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
ROUTE = REPO / "target/debug/examples/route_one_comparison"
OMEGA = REPO / "target/debug/omega"
ENV = {**dict(__import__("os").environ), "OMEGA_LIBRARY_ROOT": str(REPO / "source/library")}

Q7_PATH = Path(__file__).with_name("q7_scan_ref.py")
_q7_cache: dict[str, list] = {}


def parse_route_output(out: str) -> tuple[str, str]:
    current = route = "unknown"
    for line in out.splitlines():
        if not line.startswith("program"):
            continue
        if "root=compilation" not in line and "root=machine:" not in line:
            if "root=" in line and "root=compilation" not in line:
                continue
        mc = re.search(r"[\s\t]current=(\S+)", line)
        mr = re.search(r"[\s\t]route=(\S+)", line)
        if mc and "root=compilation" in line:
            current = mc.group(1)
        if mr and "root=compilation" in line:
            route = mr.group(1)
    if current == "unknown":
        for line in out.splitlines():
            if line.startswith("program"):
                mc = re.search(r"[\s\t]current=(\S+)", line)
                mr = re.search(r"[\s\t]route=(\S+)", line)
                if mc:
                    current = mc.group(1)
                if mr:
                    route = mr.group(1)
                break
    return current, route


def run_checker_outputs(main: Path) -> dict:
    route = subprocess.run(
        [str(ROUTE), str(main)],
        capture_output=True,
        text=True,
        timeout=120,
        env=ENV,
        stdin=subprocess.DEVNULL,
    )
    omega = subprocess.run(
        [str(OMEGA), "--check", "--offline", str(main)],
        capture_output=True,
        text=True,
        timeout=120,
        env=ENV,
        stdin=subprocess.DEVNULL,
    )
    return {
        "route_exit": route.returncode,
        "route_out": ((route.stdout or "") + (route.stderr or ""))[:4000],
        "omega_exit": omega.returncode,
        "omega_out": ((omega.stdout or "") + (omega.stderr or ""))[:4000],
        "current_verdict": parse_route_output(route.stdout or "")[0],
        "route_verdict": parse_route_output(route.stdout or "")[1],
    }


def requires_repeated_elsewhere(orig: str, removed: str) -> str | None:
    """Quote a transition/other clause that restates the removed requires."""
    pred = removed.removeprefix("requires").strip().rstrip(";")
    if not pred:
        return None
    for line in orig.splitlines():
        s = line.strip()
        if s.startswith("requires") and s != removed.strip():
            if pred in s or all(tok in s for tok in pred.split("&&") if tok.strip()):
                return s
        if "reaches" in s and pred[:20] in s:
            return s
    return None


def removed_requires_clause(orig: str, mut: str) -> str | None:
    orig_lines = orig.splitlines()
    mut_lines = mut.splitlines()
    for i, o in enumerate(orig_lines):
        if o.strip().startswith("requires") and i < len(mut_lines):
            if mut_lines[i].strip() != o.strip():
                if i >= len(mut_lines) or not mut_lines[i].strip().startswith("requires"):
                    return o.strip()
    sm = difflib.SequenceMatcher(None, orig_lines, mut_lines)
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "delete":
            chunk = "\n".join(orig_lines[i1:i2])
            if "requires" in chunk:
                for line in orig_lines[i1:i2]:
                    if line.strip().startswith("requires"):
                        return line.strip()
    return None


def drop_requires_failing_line(orig: str, mut: str, removed: str) -> str | None:
    """Body line in mutant that relied on the removed requires predicate."""
    req_vars = set(re.findall(r"[A-Za-z_][A-Za-z0-9_]*", removed))
    req_vars -= {"requires", "machine", "and", "or", "true", "false"}
    m = re.search(r"requires\s+(.+)", removed)
    if not m:
        return None
    pred = m.group(1)
    if "<" in pred or "<=" in pred:
        var = pred.split("<")[0].strip().split()[-1]
    else:
        var = next(iter(req_vars), None)
    machine_pat = re.compile(
        r"machine\s+[^\n]+\n(?:\s*(?:requires|ensures|reaches|crashes|terminates)[^\n]*\n)*\s*\{",
        re.MULTILINE,
    )
    mm = machine_pat.search(mut)
    if not mm:
        return None
    body_start = mm.end()
    body = mut[body_start:]
    depth = 1
    i = 0
    while i < len(body) and depth:
        if body[i] == "{":
            depth += 1
        elif body[i] == "}":
            depth -= 1
            if depth == 0:
                body = body[:i]
                break
        i += 1
    risky = re.compile(
        r"(\[[^\]]+\]|/\s*[^/]|%\s*[^%]|<<|>>|\+\s*[^+]|-\s*[^-]|\*\s*[^\*])"
    )
    for line in body.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("//"):
            continue
        if var and var not in line:
            continue
        if risky.search(line):
            return stripped
        if var and re.search(rf"\b{re.escape(var)}\b", line):
            if any(op in line for op in ("/", "%", "[", "+", "*", "-")):
                return stripped
    return None


def remove_mut_binding_and_line(orig: str, mut: str) -> tuple[str, str, bool] | None:
    mo = re.search(r"\blet mut\s+([A-Za-z_][A-Za-z0-9_]*)", orig)
    if not mo:
        return None
    name = mo.group(1)
    if f"let mut {name}" in mut:
        return None
    lines_mut = mut.splitlines()
    shadow = re.compile(rf"^\s*let\s+(?:mut\s+)?{re.escape(name)}\b")
    for i, line in enumerate(lines_mut, 1):
        if shadow.match(line):
            return None
        stripped = line.strip()
        if re.search(rf"&\s*(?:mut|write)\s+{re.escape(name)}\b", line):
            return name, stripped, False
        if re.search(rf"^\s*{re.escape(name)}\.", line) or re.search(
            rf"^\s*{re.escape(name)}\[", line
        ):
            if any(op in line for op in ("=", "+=", "-=", "*=", "/=")):
                return name, stripped, False
        if re.search(rf"^\s*{re.escape(name)}\s*=", line) and "let " not in line:
            return name, stripped, False
    return None


def load_q7_hits(main_path: Path) -> list:
    key = str(main_path)
    if key in _q7_cache:
        return _q7_cache[key]
    import sys

    spec = importlib.util.spec_from_file_location("q7_scan_ref", Q7_PATH)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules["q7_scan_ref"] = mod
    spec.loader.exec_module(mod)
    hits = mod.scan_file(main_path)
    _q7_cache[key] = hits
    return hits


def is_known_q7_gap(
    fixture_main: Path, binding: str, obligation_line: str
) -> tuple[bool, str]:
    hits = load_q7_hits(fixture_main)
    for h in hits:
        if h.binding != binding:
            continue
        if binding not in obligation_line:
            continue
        return True, h.snippet
    return False, ""


SPEC_QUOTES = {
    "drop_machine_requires_arith": (
        "wiki/spec/language/numeric_values.md",
        "## Exact operations",
        "`+`, `*`: Exact integer result, without numerical overflow.",
    ),
    "drop_machine_requires_index": (
        "wiki/spec/language/domains.md",
        "## Indexed domains",
        "Indexed access requires a proof that the index lies in the domain established for that storage.",
    ),
    "remove_mut": (
        "wiki/spec/language/ownership.md",
        "## Owned local mutability",
        "modifying an established ordinary local's owned storage requires a `mut` binding",
    ),
    "widen_requires_lt": (
        "wiki/spec/language/state_contracts.md",
        "## Requirement clauses",
        "Machine | Entry obligations established at every invocation under exact actual-argument substitution.",
    ),
}


def pick_spec_quote(kind: str, obligation_line: str | None) -> tuple[str, str, str] | None:
    if kind == "remove_mut":
        return SPEC_QUOTES["remove_mut"]
    if kind == "widen_requires_lt":
        return SPEC_QUOTES["widen_requires_lt"]
    if kind == "drop_machine_requires" and obligation_line:
        if any(x in obligation_line for x in ("/", "%", "+", "*", "-", "<<", ">>")):
            return SPEC_QUOTES["drop_machine_requires_arith"]
        if "[" in obligation_line:
            return SPEC_QUOTES["drop_machine_requires_index"]
    return None
