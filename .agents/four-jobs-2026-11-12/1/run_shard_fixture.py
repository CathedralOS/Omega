#!/usr/bin/env python3
"""Run route_one_comparison on one corpus fixture; emit one TSV row."""

from __future__ import annotations

import re
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path


HARNESS = Path(__file__).resolve().parents[3] / "target/debug/examples/route_one_comparison"
TIMEOUT_SEC = 300

MACHINE_IN_DIAG = re.compile(
    r"(?:machine|in machine)\s+`([^`]+)`", re.IGNORECASE
)
MACHINE_ROOT = re.compile(r"^machine:(.+)$")


def tier(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


@dataclass
class ProgramRoot:
    root: str
    current: str
    route: str
    refusal_detail: str
    fields: list[str]


def field_value(fields: list[str], prefix: str) -> str:
    for f in fields:
        if f.startswith(prefix):
            return f[len(prefix) :]
    return ""


def refusal_construct(route: str, detail: str, fields: list[str]) -> str:
    if route == "no-entry":
        return "meta:no-program-entry"
    if route == "unreached":
        return "meta:typed-frontier-stopped"
    if route == "refused-module":
        reason = field_value(fields, "reason=")
        return reason if reason else "refused-module"
    if route == "fallback":
        return detail if detail else "fallback"
    if detail:
        if ": " in detail:
            _kind, spec = detail.split(": ", 1)
            return spec.strip() or detail.strip()
        return detail.strip()
    if route in ("defect", "panicked", "incomplete"):
        return route
    return route if route else "outside"


def classify_route(route: str) -> str:
    base = route.split()[0]
    if base == "accepted":
        return "accepted"
    if base in ("rejected", "incomplete"):
        return "rejected"
    if base == "outside" or base.startswith("outside"):
        return "outside"
    if base in (
        "fallback",
        "refused-module",
        "defect",
        "panicked",
        "no-entry",
        "unreached",
    ):
        return "outside"
    return "outside"


def parse_program_lines(
    text: str, fixture: str
) -> tuple[list[ProgramRoot], list[tuple[str, str, str]], list[str]]:
    programs: list[ProgramRoot] = []
    fallbacks: list[tuple[str, str, str]] = []
    current_diags: list[str] = []
    for line in text.splitlines():
        if not line.strip():
            continue
        fields = line.split("\t")
        kind = fields[0]
        if kind == "program" and len(fields) > 1 and fields[1] == fixture:
            current = field_value(fields, "current=")
            root = field_value(fields, "root=")
            route = ""
            detail = ""
            for i, f in enumerate(fields):
                if f == "route=outside" and i + 1 < len(fields):
                    route = "outside"
                    detail = fields[i + 1]
                    break
                if f.startswith("route="):
                    route = f[len("route=") :]
            programs.append(
                ProgramRoot(
                    root=root,
                    current=current,
                    route=route,
                    refusal_detail=detail,
                    fields=fields,
                )
            )
        elif kind == "fallback" and len(fields) > 1 and fields[1] == fixture:
            root = field_value(fields, "root=")
            construct = fields[4].strip() if len(fields) > 4 else ""
            detail = fields[5].strip() if len(fields) > 5 else ""
            fallbacks.append((root, construct, detail))
        elif kind == "current-diagnostic" and len(fields) > 1 and fields[1] == fixture:
            msg = fields[-1] if len(fields) > 3 else ""
            current_diags.append(msg)
    return programs, fallbacks, current_diags


def diagnostic_class(message: str) -> str:
    m = message.lower()
    if "borrow" in m or "loan" in m or "mutates" in m and "while" in m:
        return "borrow/lifetime"
    if "shift count" in m or "shift" in m and "out of range" in m:
        return "shift/bounds"
    if "overflow" in m or "exact arithmetic" in m or "out of range" in m:
        return "arithmetic/bounds"
    if "ensures" in m or "requires" in m or "cannot prove" in m and "contract" in m:
        return "contract/proof"
    if "division" in m or "index" in m or "bounds" in m:
        return "operation-safety"
    if "build" in m or "programentry" in m or "entry" in m and "bind" in m:
        return "build/entry"
    if "statement" in m and ("crash" in m or "discard" in m):
        return "effects/discipline"
    if not message.strip() or message == "no-span":
        return "unpositioned/other"
    return "other"


def first_diagnostic_class(current_diags: list[str]) -> str:
    if not current_diags:
        return "no-diagnostic"
    return diagnostic_class(current_diags[0])


def machines_named_in_diagnostics(current_diags: list[str]) -> set[str]:
    found: set[str] = set()
    for msg in current_diags:
        for match in MACHINE_IN_DIAG.finditer(msg):
            found.add(match.group(1))
    return found


def tally_aggregate(programs: list[ProgramRoot]) -> str:
    routes = [p.route for p in programs if p.route]
    if not routes:
        return "error"
    has_rejected = any(classify_route(r) == "rejected" for r in routes)
    has_outside = any(classify_route(r) == "outside" for r in routes)
    has_accepted = any(classify_route(r) == "accepted" for r in routes)
    if has_rejected:
        return "rejected"
    if has_accepted and has_outside:
        return "accepted"
    if has_outside:
        return "outside"
    if has_accepted:
        return "accepted"
    return "outside"


def fail_skipped_rejection_root(
    programs: list[ProgramRoot], current_diags: list[str]
) -> bool:
    """True when current rejects but at least one relevant root was not fully judged."""
    compilation_no_entry = any(
        p.root == "compilation" and p.route == "no-entry" for p in programs
    )
    machine_roots = [p for p in programs if p.root.startswith("machine:")]
    has_outside = any(classify_route(p.route) == "outside" for p in programs)
    if compilation_no_entry and machine_roots:
        return True
    if has_outside:
        return True
    judged = {p.root for p in machine_roots}
    judged_names = set()
    for p in machine_roots:
        m = MACHINE_ROOT.match(p.root)
        if m:
            judged_names.add(m.group(1))
    for name in machines_named_in_diagnostics(current_diags):
        if f"machine:{name}" not in judged and name not in judged_names:
            return True
    # Current diagnostic names a machine whose closure is outside/unreached only
    for p in machine_roots:
        if classify_route(p.route) == "outside":
            return True
    if not machine_roots and compilation_no_entry:
        return True
    return False


def program_route_verdict(
    programs: list[ProgramRoot], fallbacks: list[tuple[str, str, str]], fixture: str, current_diags: list[str]
) -> str:
    current = programs[0].current or "unknown"
    tally = tally_aggregate(programs)

    if tier(fixture) != "fail":
        return tally

    if current == "rejected" and tally == "accepted":
        if fail_skipped_rejection_root(programs, current_diags):
            return "partial"
    return tally


def first_refusal(
    programs: list[ProgramRoot],
    fallbacks: list[tuple[str, str, str]],
    fixture: str,
) -> str:
    skip_entry_no_entry = tier(fixture) == "fail" and any(
        p.root.startswith("machine:") for p in programs
    )
    fallback_by_root = {r: (c, d) for r, c, d in fallbacks}
    for prog in programs:
        if (
            skip_entry_no_entry
            and prog.root == "compilation"
            and prog.route == "no-entry"
        ):
            continue
        base = prog.route.split()[0] if prog.route else ""
        if base in ("outside", "no-entry", "unreached", "fallback", "refused-module"):
            if base == "fallback" and prog.root in fallback_by_root:
                c, d = fallback_by_root[prog.root]
                return refusal_construct("fallback", f"{c}: {d}".strip(": "), prog.fields)
            return refusal_construct(base, prog.refusal_detail, prog.fields)
        if base == "incomplete":
            return "incomplete-proof-rule"
        if base in ("defect", "panicked"):
            return base
    for _root, construct, detail in fallbacks:
        return refusal_construct("fallback", f"{construct}: {detail}".strip(": "), [])
    return ""


def agreement(route_verdict: str, current_verdict: str) -> str:
    if route_verdict == "partial":
        return "n-a"
    if route_verdict == "outside":
        return "n-a"
    if route_verdict == "accepted" and current_verdict == "accepted":
        return "agree"
    if route_verdict == "rejected" and current_verdict in ("rejected", "stopped"):
        return "agree"
    if route_verdict in ("accepted", "rejected") and current_verdict in (
        "accepted",
        "rejected",
        "stopped",
    ):
        return "disagree"
    return "n-a"


def parse_output(text: str, fixture: str) -> tuple[str, str, str, str]:
    programs, fallbacks, current_diags = parse_program_lines(text, fixture)
    if not programs:
        return "error", "unknown", "n-a", "no-program-line"

    current_verdict = programs[0].current or "unknown"
    route_verdict = program_route_verdict(programs, fallbacks, fixture, current_diags)
    refusal = first_refusal(programs, fallbacks, fixture)
    agree = agreement(route_verdict, current_verdict)
    return route_verdict, current_verdict, agree, refusal


def main() -> None:
    if len(sys.argv) != 2:
        print("usage: run_shard_fixture.py <fixture-path>", file=sys.stderr)
        sys.exit(2)
    fixture = sys.argv[1]
    cmd = [str(HARNESS)]
    if tier(fixture) == "fail":
        cmd.append("--every-machine")
    cmd.append(fixture)
    t0 = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_SEC,
            stdin=subprocess.DEVNULL,
        )
        elapsed = time.perf_counter() - t0
        text = proc.stdout
        if proc.returncode != 0 and not text.strip():
            text = proc.stderr + proc.stdout
        if "panicked" in text and "program\t" not in text:
            print("\t".join((fixture, tier(fixture), "error", "unknown", "n-a", "panicked", f"{elapsed:.3f}")))
            return
        route_v, cur_v, agree, refusal = parse_output(text, fixture)
        print("\t".join((fixture, tier(fixture), route_v, cur_v, agree, refusal, f"{elapsed:.3f}")))
    except subprocess.TimeoutExpired:
        elapsed = time.perf_counter() - t0
        print("\t".join((fixture, tier(fixture), "timeout", "unknown", "n-a", "", f"{elapsed:.3f}")))


if __name__ == "__main__":
    main()
