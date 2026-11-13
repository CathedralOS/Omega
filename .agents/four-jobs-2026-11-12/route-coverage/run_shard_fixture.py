#!/usr/bin/env python3
"""Run route_one_comparison on one corpus fixture; emit one TSV row."""

from __future__ import annotations

import re
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path


REPO = Path(__file__).resolve().parents[3]
HARNESS = REPO / "target/debug/examples/route_one_comparison"
TIMEOUT_SEC = 300
RAW_DIR = Path("/tmp/route-coverage-shard-2")

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
    obligations: int
    judged: int


def field_value(fields: list[str], prefix: str) -> str:
    for f in fields:
        if f.startswith(prefix):
            return f[len(prefix) :]
    return ""


def int_field(fields: list[str], prefix: str) -> int:
    raw = field_value(fields, prefix)
    try:
        return int(raw) if raw else 0
    except ValueError:
        return 0


def normalize_root_verdict(route: str) -> str:
    if not route:
        return "error"
    base = route.split()[0]
    if base == "accepted":
        return "accepted"
    if base in ("rejected", "incomplete"):
        return "rejected"
    if base == "no-entry":
        return "no-entry"
    if base == "unreached":
        return "outside"
    if base in ("outside",) or base.startswith("outside"):
        return "outside"
    if base in ("fallback", "refused-module", "defect", "panicked"):
        return "outside"
    return "outside"


def root_label(prog: ProgramRoot) -> str:
    if prog.root == "compilation":
        return "compilation"
    m = MACHINE_ROOT.match(prog.root)
    if m:
        return f"machine:{m.group(1)}"
    return prog.root.replace(":", "_")


def format_roots(programs: list[ProgramRoot]) -> str:
    parts = []
    for prog in programs:
        v = normalize_root_verdict(prog.route)
        parts.append(f"{root_label(prog)}:{v}")
    return ";".join(parts)


def vacuous_accept_roots(programs: list[ProgramRoot]) -> str:
    vacuous = []
    for prog in programs:
        if (
            normalize_root_verdict(prog.route) == "accepted"
            and prog.obligations == 0
            and prog.judged == 0
        ):
            vacuous.append(root_label(prog))
    return ";".join(vacuous)


def program_verdict_from_roots(root_verdicts: list[str]) -> str:
    if not root_verdicts:
        return "error"
    has_rejected = any(v == "rejected" for v in root_verdicts)
    has_accepted = any(v == "accepted" for v in root_verdicts)
    has_skipped = any(v in ("outside", "no-entry", "error") for v in root_verdicts)
    if has_rejected:
        return "rejected"
    if has_accepted and has_skipped:
        return "partial"
    if has_accepted and all(v == "accepted" for v in root_verdicts):
        return "accepted"
    if has_accepted:
        return "accepted"
    return "outside"


def refusal_construct(route: str, detail: str, fields: list[str]) -> str:
    if route == "no-entry":
        return "meta:no-program-entry"
    if route == "unreached":
        return "meta:typed-frontier-stopped"
    if route == "refused-module":
        reason = field_value(fields, "reason=")
        return reason if reason else "refused-module"
    if detail:
        if ": " in detail:
            _kind, spec = detail.split(": ", 1)
            return spec.strip() or detail.strip()
        return detail.strip()
    if route in ("defect", "panicked", "incomplete"):
        return route
    return ""


def first_refusal(
    programs: list[ProgramRoot],
    fixture: str,
) -> str:
    skip_entry = tier(fixture) == "fail" and any(
        p.root.startswith("machine:") for p in programs
    )
    for prog in programs:
        if skip_entry and prog.root == "compilation" and prog.route == "no-entry":
            continue
        base = prog.route.split()[0] if prog.route else ""
        if base in ("outside", "no-entry", "unreached", "fallback", "refused-module"):
            return refusal_construct(base, prog.refusal_detail, prog.fields)
        if base == "incomplete":
            return "incomplete-proof-rule"
    return ""


def agreement(program_verdict: str, current_verdict: str) -> str:
    if program_verdict in ("partial", "outside", "timeout", "error"):
        return "n-a"
    if program_verdict == "accepted" and current_verdict == "accepted":
        return "agree"
    if program_verdict == "rejected" and current_verdict in ("rejected", "stopped"):
        return "agree"
    if program_verdict in ("accepted", "rejected") and current_verdict in (
        "accepted",
        "rejected",
        "stopped",
    ):
        return "disagree"
    return "n-a"


def parse_program_lines(
    text: str, fixture: str
) -> list[ProgramRoot]:
    programs: list[ProgramRoot] = []
    for line in text.splitlines():
        if not line.strip():
            continue
        fields = line.split("\t")
        if fields[0] != "program" or len(fields) < 2 or fields[1] != fixture:
            continue
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
                root=field_value(fields, "root="),
                current=field_value(fields, "current="),
                route=route,
                refusal_detail=detail,
                fields=fields,
                obligations=int_field(fields, "obligations="),
                judged=int_field(fields, "judged="),
            )
        )
    return programs


def parse_output(text: str, fixture: str) -> tuple[str, str, str, str, str, str]:
    programs = parse_program_lines(text, fixture)
    if not programs:
        return "error", "unknown", "n-a", "no-program-line", "", ""

    current_verdict = programs[0].current or "unknown"
    root_verdicts = [normalize_root_verdict(p.route) for p in programs]
    program_verdict = program_verdict_from_roots(root_verdicts)
    refusal = first_refusal(programs, fixture)
    agree = agreement(program_verdict, current_verdict)
    roots = format_roots(programs)
    vacuous = vacuous_accept_roots(programs)
    return program_verdict, current_verdict, agree, refusal, roots, vacuous


def save_raw(fixture: str, text: str) -> None:
    RAW_DIR.mkdir(parents=True, exist_ok=True)
    safe = fixture.replace("/", "__")
    (RAW_DIR / f"{safe}.txt").write_text(text)


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
            cwd=str(REPO),
        )
        elapsed = time.perf_counter() - t0
        text = proc.stdout
        if proc.returncode != 0 and not text.strip():
            text = proc.stderr + proc.stdout
        save_raw(fixture, text)
        if "panicked" in text and "program\t" not in text:
            print(
                "\t".join(
                    (
                        fixture,
                        tier(fixture),
                        "error",
                        "unknown",
                        "n-a",
                        "panicked",
                        f"{elapsed:.3f}",
                        "",
                        "",
                    )
                )
            )
            return
        p_v, cur_v, agree, refusal, roots, vacuous = parse_output(text, fixture)
        print(
            "\t".join(
                (
                    fixture,
                    tier(fixture),
                    p_v,
                    cur_v,
                    agree,
                    refusal,
                    f"{elapsed:.3f}",
                    roots,
                    vacuous,
                )
            )
        )
    except subprocess.TimeoutExpired:
        elapsed = time.perf_counter() - t0
        print(
            "\t".join(
                (
                    fixture,
                    tier(fixture),
                    "timeout",
                    "unknown",
                    "n-a",
                    "",
                    f"{elapsed:.3f}",
                    "",
                    "",
                )
            )
        )


if __name__ == "__main__":
    main()
