#!/usr/bin/env python3
"""Lead-review helpers for soundness mutation shard confirmation (fix3)."""
from __future__ import annotations

import difflib
import importlib.util
import re
import shutil
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
ROUTE = REPO / "target/debug/examples/route_one_comparison"
OMEGA = REPO / "target/debug/omega"
ENV = {**dict(__import__("os").environ), "OMEGA_LIBRARY_ROOT": str(REPO / "source/library")}

Q7_PATH = Path(__file__).with_name("q7_scan_ref.py")
_q7_cache: dict[str, list] = {}

ROUTE_ACCEPT = {"accepted"}
ROUTE_REJECT = {
    "rejected",
    "incomplete",
    "defect",
    "outside",
    "fallback",
    "no-entry",
    "unreached",
    "error",
    "unknown",
}


ROUTE_NON_VERDICT = frozenset({"outside", "no-entry", "unreached"})


def accepted_by(current: str, route: str) -> str:
    """Fix4: only route=accepted counts as route acceptance."""
    cur = current == "accepted"
    ro = route == "accepted"
    if cur and ro:
        return "both"
    if cur:
        return "current"
    if ro:
        return "route"
    return "neither"


def route_rejects(route: str) -> bool:
    return route in ("rejected", "incomplete", "defect", "fallback")


def current_rejects(current: str) -> bool:
    return current in ("rejected", "stopped")


def parse_route_output(out: str) -> tuple[str, str]:
    current = route = "unknown"
    for line in out.splitlines():
        if not line.startswith("program"):
            continue
        mc = re.search(r"[\s\t]current=(\S+)", line)
        mr = re.search(r"[\s\t]route=(\S+)", line)
        if mc:
            current = mc.group(1)
        if mr:
            route = mr.group(1)
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
    pred = removed.removeprefix("requires").strip().rstrip(";")
    if not pred:
        return None
    for line in orig.splitlines():
        s = line.strip()
        if s.startswith("requires") and s != removed.strip():
            if pred in s:
                return s
    return None


def removed_requires_clause(orig: str, mut: str) -> str | None:
    orig_lines = orig.splitlines()
    mut_lines = mut.splitlines()
    sm = difflib.SequenceMatcher(None, orig_lines, mut_lines)
    for tag, i1, i2, j1, j2 in sm.get_opcodes():
        if tag == "delete":
            for line in orig_lines[i1:i2]:
                if line.strip().startswith("requires"):
                    return line.strip()
    return None


def drop_requires_failing_line(orig: str, mut: str, removed: str) -> str | None:
    """Body or ensures line in mutant that still relies on the removed requires fact."""
    m = re.search(r"requires\s+(.+)", removed)
    if not m:
        return None
    pred = m.group(1)
    tokens = {t for t in re.findall(r"[A-Za-z_][A-Za-z0-9_]*", pred) if t not in ("requires", "and", "or", "true", "false")}
    for line in mut.splitlines():
        s = line.strip()
        if not s or s.startswith("//") or s.startswith("ensures"):
            continue
        if not any(t in s for t in tokens):
            continue
        if any(op in s for op in ("[", "]", "/", "%", "+", "*", "-", "<<", ">>", "as ")):
            return s
        if s.startswith("ensures"):
            return s
    for line in mut.splitlines():
        if line.strip().startswith("ensures") and any(t in line for t in tokens):
            return line.strip()
    return None


def remove_mut_binding_and_line(orig: str, mut: str) -> tuple[str, str] | None:
    mo = re.search(r"\blet mut\s+([A-Za-z_][A-Za-z0-9_]*)", orig)
    if not mo:
        return None
    name = mo.group(1)
    shadow = re.compile(rf"^\s*let\s+(?:mut\s+)?{re.escape(name)}\b")
    for line in mut.splitlines():
        if shadow.match(line):
            return None
    for line in mut.splitlines():
        stripped = line.strip()
        if re.search(rf"&\s*(?:mut|write)\s+{re.escape(name)}\b", line):
            return name, stripped
        if re.search(rf"^\s*{re.escape(name)}\.", line) or re.search(
            rf"^\s*{re.escape(name)}\[", line
        ):
            if any(op in line for op in ("=", "+=", "-=", "*=", "/=")):
                return name, stripped
        if re.search(rf"^\s*{re.escape(name)}\s*=", line) and "let " not in line:
            return name, stripped
    return None


def ref_mutation_obligation(orig: str, mut: str, kind: str) -> tuple[str, str] | None:
    diff_lines = [
        b
        for a, b in zip(orig.splitlines(), mut.splitlines())
        if a != b and ("&" in a or "&" in b)
    ]
    if not diff_lines:
        return None
    line = diff_lines[0].strip()
    if kind == "ref_to_mut_ref":
        return line, "&mut formed where the source only authorized shared `&`"
    if kind == "mut_ref_to_ref":
        if re.search(r"\.[A-Za-z_]+\s*=", mut) or re.search(r"\[[^\]]+\]\s*=", mut):
            for wl in mut.splitlines():
                if "=" in wl and "&" not in wl.split("=")[0][-3:]:
                    if wl.strip() and not wl.strip().startswith("//"):
                        return wl.strip(), "write through shared `&` where `&mut` was required"
        return line, "downgraded `&mut` to `&` while exclusive use remains"
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


def is_known_q7_gap(fixture_main: Path, binding: str, obligation_line: str) -> tuple[bool, str]:
    if binding not in obligation_line:
        return False, ""
    for h in load_q7_hits(fixture_main):
        if h.binding == binding:
            return True, h.snippet
    return False, ""


SPEC_RULES = {
    "remove_mut": (
        "wiki/spec/language/ownership.md",
        "Owned local mutability",
        "modifying an established ordinary local's owned storage requires a `mut` binding",
    ),
    "drop_requires_arith": (
        "wiki/spec/language/numeric_values.md",
        "Exact operations",
        "Division or remainder by zero must be excluded before formation.",
    ),
    "drop_requires_bound": (
        "wiki/spec/language/state_contracts.md",
        "Requirement clauses",
        "Machine entry obligations are established at every invocation under exact actual-argument substitution.",
    ),
    "drop_requires_modular": (
        "wiki/spec/language/machines.md",
        "Nested machines and identity",
        (
            "A written entry contract remains complete even when all currently visible callers happen "
            "to establish stronger facts. "
            "An authored `requires` is complete, including on a private helper; inference cannot "
            "silently conjoin further entry conditions to make the body check "
            "(wiki/spec/language/state_contracts.md, Private entry inference)."
        ),
    ),
    "ref_to_mut_ref": (
        "wiki/spec/language/ownership.md",
        "Owned local mutability",
        "formation of `&mut` or `&write` loans to owned places requires a `mut` binding on that carrier when lending owned storage",
    ),
    "mut_ref_to_ref": (
        "wiki/spec/language/ownership.md",
        "Borrowing",
        "Exclusive mutation requires a `&mut` loan; shared `&` does not authorize writes to the referent",
    ),
    "widen_requires": (
        "wiki/spec/language/state_contracts.md",
        "Requirement clauses",
        "Entry obligations must hold for every admitted invocation; widening `<` to `<=` admits values that violate the original bound.",
    ),
}


CHECK_TIME_MARKERS = re.compile(
    r"cannot prove|proof obligation|must be excluded before|requires a `mut` binding",
    re.I,
)
RUNTIME_TRAP_MARKERS = re.compile(
    r"crashes?\s|reaches\s+\w*[Tt]rap|runtime guard|crash route",
    re.I,
)


def acceptance_mechanism(mut_path: Path) -> str:
    """Fix4(b): whether acceptance is check-time or defers to runtime trap."""
    out = run_checker_outputs(mut_path)
    if out["omega_exit"] != 0 and out["current_verdict"] != "accepted":
        return "rejected"
    text = out["omega_out"] + "\n" + out["route_out"]
    if RUNTIME_TRAP_MARKERS.search(text) and not CHECK_TIME_MARKERS.search(text):
        return "runtime_trap"
    if CHECK_TIME_MARKERS.search(text):
        return "check_time_violation_unreported"
    return "check_time_pass"


def machine_after_requires_drop(orig: str, mut: str, removed: str) -> tuple[str, str] | None:
    """Return (machine_name, params_text) for the machine that lost `removed`."""
    lines_o = orig.splitlines()
    for i, line in enumerate(lines_o):
        if line.strip() == removed and i > 0:
            hdr = lines_o[i - 1].strip()
            m = re.match(r"machine\s+(\w+)\s*\(([^)]*)\)", hdr)
            if m:
                return m.group(1), m.group(2)
    return None


def violating_argument(requires: str) -> str | None:
    req = requires.removeprefix("requires").strip()
    m = re.search(r"(\w+)\s*<\s*(\w+)", req)
    if m:
        return m.group(2)
    m = re.search(r"(\w+)\s*<=\s*(\w+)", req)
    if m:
        bound = m.group(2)
        if bound.isdigit():
            return str(int(bound) + 1)
        return bound
    m = re.search(r"(\w+)\s*>\s*(\w+)", req)
    if m:
        return m.group(2)
    return None


def probe_drop_requires_subtype(mut_path: Path, orig_path: Path, removed: str) -> tuple[str, str]:
    """fix5: inferred-entry vs unchecked via violating-caller probe."""
    orig = orig_path.read_text()
    mut = mut_path.read_text()
    info = machine_after_requires_drop(orig, mut, removed)
    if not info:
        return "unchecked", "could not locate machine for probe"
    name, params = info
    bad = violating_argument(removed)
    if not bad:
        return "unchecked", "could not synthesize violating argument"
    arity = len([p for p in params.split(",") if p.strip()]) or 1
    args = [bad] + ["0"] * max(0, arity - 1)
    call = f"{name}({', '.join(args[:arity])})"
    probe_body = (
        f"\n\nmachine __soundness_probe_violation__() -> u64\n{{\n    {call}\n}}\n"
    )
    probe_dir = mut_path.parent / "probe_violation"
    probe_dir.mkdir(exist_ok=True)
    for item in mut_path.parent.iterdir():
        if item.suffix == ".omg" and item.name != "main.omg":
            shutil.copy2(item, probe_dir / item.name)
    (probe_dir / "main.omg").write_text(mut + probe_body)
    probe_main = probe_dir / "main.omg"
    out = run_checker_outputs(probe_main)
    mut_ok = run_checker_outputs(mut_path)["current_verdict"] == "accepted"
    probe_ok = out["current_verdict"] == "accepted"
    note = f"probe call `{call}` omega_exit={out['omega_exit']}"
    if mut_ok and not probe_ok:
        return "inferred-entry", note
    if mut_ok and probe_ok:
        return "unchecked", note
    return "unchecked", note + "; mutant not accepted on re-check"


def minimal_mutant_diff(orig: str, mut: str, max_lines: int = 25) -> str:
    import difflib

    return "\n".join(
        difflib.unified_diff(
            orig.splitlines(),
            mut.splitlines(),
            fromfile="original",
            tofile="mutant",
            lineterm="",
            n=2,
        )
    )[:4000]


def read_mutant_validity(
    orig: str, mut: str, kind: str, fixture_main: Path
) -> tuple[bool, str, str, str, str]:
    """
    Return (is_invalid, one_line_reason, obligation_line, rule_path, rule_quote).
    Validity from reading the mutant only (fix3-B).
    """
    if kind == "drop_machine_requires":
        removed = removed_requires_clause(orig, mut)
        if not removed:
            return True, "requires removed but diff not parsed", "", "", ""
        fail = drop_requires_failing_line(orig, mut, removed)
        if not fail:
            return (
                False,
                "no body or ensures line in the mutant still depends on the removed requires",
                "",
                "",
                "",
            )
        if any(c in fail for c in "/%+-*[]"):
            path, heading, quote = SPEC_RULES["drop_requires_arith"]
        else:
            path, heading, quote = SPEC_RULES["drop_requires_modular"]
        return (
            True,
            f"modular entry: body `{fail}` needs `{removed}`; caller re-establishment cannot make this valid",
            fail,
            path,
            quote,
        )

    if kind == "widen_requires_lt":
        fail = drop_requires_failing_line(orig, mut, "requires widened")
        path, _, quote = SPEC_RULES["widen_requires"]
        return (
            True,
            "widened bound admits calls that violate the original requires",
            fail or "(widened requires clause)",
            path,
            quote,
        )

    if kind == "remove_mut":
        ev = remove_mut_binding_and_line(orig, mut)
        if not ev:
            return False, "demoted `mut` does not pair with a write or &mut on that binding", "", "", ""
        binding, line = ev
        path, _, quote = SPEC_RULES["remove_mut"]
        return (
            True,
            f"`{binding}` is used mutably without a `mut` binding at `{line}`",
            line,
            path,
            quote,
        )

    if kind in ("ref_to_mut_ref", "mut_ref_to_ref"):
        ev = ref_mutation_obligation(orig, mut, kind)
        if not ev:
            return False, "reference change is not tied to an exclusive-use obligation line", "", "", ""
        line, reason = ev
        key = kind
        path, _, quote = SPEC_RULES[key]
        return True, reason, line, path, quote

    if kind == "drop_machine_ensures":
        return False, "dropping ensures weakens the guarantee; program may remain spec-valid", "", "", ""

    return False, f"{kind} not classified for validity read", "", "", ""
