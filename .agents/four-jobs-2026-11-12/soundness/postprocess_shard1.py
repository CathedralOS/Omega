#!/usr/bin/env python3
"""Triage shard-1.tsv per lead soundness_fix3 (wins over fix + fix2)."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

_HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE))
from run_shard1 import extract_route_reason

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 1
FIXTURE_TOTAL = 1131
OUT = Path(__file__).resolve().parent
TSV = OUT / f"shard-{SHARD}.tsv"
MD = OUT / f"shard-{SHARD}.md"
Q7 = OUT / "q7_fixture_paths.txt"
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
ENV = {"OMEGA_LIBRARY_ROOT": str(REPO / "source/library")}

BUCKETS = (
    "confirmed-hole",
    "known-gap-q7-mut",
    "disagreement-valid",
    "discarded-valid",
    "rejected-both",
    "timeout",
    "failed-run",
    "no-mutation",
)


def q7_fixtures() -> set[str]:
    if not Q7.is_file():
        return set()
    out: set[str] = set()
    for ln in Q7.read_text().splitlines():
        ln = ln.strip().strip("-").strip().strip("`")
        if ln:
            out.add(ln)
    return out


def read_rows() -> tuple[list[str], list[dict[str, str]]]:
    lines = TSV.read_text(encoding="utf-8").splitlines()
    if not lines:
        return [], []
    hdr = lines[0].split("\t")
    rows = []
    for line in lines[1:]:
        cols = line.split("\t")
        if len(cols) < len(hdr):
            cols += [""] * (len(hdr) - len(cols))
        rows.append(dict(zip(hdr, cols)))
    return hdr, rows


def route_accepts(route: str) -> bool:
    return route == "accepted"


def current_accepts(cur: str) -> bool:
    return cur == "accepted"


def accepted_by(cur: str, route: str) -> str:
    c, r = current_accepts(cur), route_accepts(route)
    if c and r:
        return "both"
    if c:
        return "current"
    if r:
        return "route"
    return ""


def first_requires_predicate(source: str) -> str:
    m = re.search(r"^(\s*)requires\b([^\n]*)\n", source, re.MULTILINE)
    return m.group(2).strip() if m else ""


def requires_duplicated_in_transition(source: str) -> tuple[bool, str, str]:
    req = first_requires_predicate(source)
    if not req:
        return False, "", ""
    for m in re.finditer(r"transition\s+([^\n{]+)", source):
        guard = m.group(1).strip()
        if req == guard or req in guard or guard in req:
            return True, req, guard
        for frag in re.findall(r"[\w().:]+\s*[=<>!]+\s*[\w().:]+", req):
            if frag.replace(" ", "") in guard.replace(" ", ""):
                return True, req, guard
    return False, "", ""


def machine_body_after_first_requires(source: str) -> str:
    m = re.search(r"^(\s*)requires\b[^\n]*\n", source, re.MULTILINE)
    if not m:
        return ""
    chunk = source[m.end() :]
    end = re.search(r"^\s*machine\s+", chunk, re.MULTILINE)
    return chunk[: end.start()] if end else chunk


def read_mutant_validity(
    kind: str, original: str, mutant: str
) -> tuple[bool, str, str, str, str]:
    """Returns (spec_invalid, obligation_line, spec_file, spec_quote, validity_note)."""
    note = ""

    if kind == "drop_requires":
        dup, rq, tq = requires_duplicated_in_transition(original)
        if dup:
            return False, "", "", "", f"valid: requires `{rq}` restated in transition `{tq}`"
        block = machine_body_after_first_requires(mutant)
        for line in block.splitlines():
            s = line.strip()
            if s.startswith("ensures "):
                dropped = first_requires_predicate(original)
                return (
                    True,
                    s,
                    "wiki/spec/language/state_contracts.md",
                    "Every invocation retains both sides of its own contract.",
                    f"ensures remains after dropping requires `{dropped}`",
                )
            if s and s not in ("{", "}"):
                if re.search(r"/\s*[^/\s]|\[\s*\w|embed\(|\.len\b", s):
                    return (
                        True,
                        s,
                        "wiki/spec/language/state_contracts.md",
                        "Machine `requires` applies at every invocation, including terminal and recursive calls.",
                        "body operation still present without entry requires",
                    )
        return False, "", "", "", "valid: no ensures/body line needs dropped requires"

    if kind == "remove_mut":
        m = re.search(r"\blet mut (\w+)", original)
        if not m:
            return False, "", "", "", "valid: no let mut in original"
        name = m.group(1)
        if re.search(rf"\blet(?:\s+mut)?\s+{re.escape(name)}\b", original[m.end() :]):
            return False, "", "", "", "valid: shadowed binding"
        for line in original.splitlines():
            if re.search(
                rf"(?:&mut\s+{re.escape(name)}\b|&write\s+{re.escape(name)}\b|{re.escape(name)}\s*=(?!=)|{re.escape(name)}\[[^\]]+\]\s*=|{re.escape(name)}\.\w+\s*=)",
                line,
            ):
                return (
                    True,
                    line.strip(),
                    "wiki/spec/language/ownership.md",
                    "modifying an established ordinary local's owned storage requires a `mut` binding.",
                    "remove_mut on binding that is written or lent mutably",
                )
        return False, "", "", "", "valid: first let mut has no write/&mut on same binding"

    if kind == "amp_to_ampmut":
        for line in mutant.splitlines():
            if "&mut" in line and "&" in line.replace("&mut", ""):
                return (
                    True,
                    line.strip(),
                    "wiki/spec/language/expressions.md",
                    "Such a loan requires existing writable authority at the selected place;",
                    "&mut without writable authority on carrier",
                )
        return False, "", "", "", "valid: amp_to_ampmut not substantiated in mutant"

    if kind == "remove_guard_transition":
        m = re.search(r"^\s*transition\s+([^\n]+)\{\s*\n", original, re.MULTILINE)
        if not m:
            return False, "", "", "", "valid: no transition guard removed"
        guard = m.group(1).strip()
        if guard.lower() in ("true", "false"):
            return False, "", "", "", "valid: trivial guard"
        rest = original[m.end() :]
        arm = next((ln.strip() for ln in rest.splitlines() if ln.strip() and not ln.strip().startswith("machine")), "")
        return (
            True,
            f"removed guard `{guard}` before `{arm}`",
            "wiki/spec/language/state_contracts.md",
            "Only the selected arm executes, with ordinary once-only operand evaluation.",
            "unguarded arm may run without establishing guard facts",
        )

    if kind == "widen_requires":
        om = re.search(r"requires\b[^\n]+", original)
        mm = re.search(r"requires\b[^\n]+", mutant)
        if om and mm and om.group(0) != mm.group(0):
            return (
                True,
                f"{om.group(0).strip()} -> {mm.group(0).strip()}",
                "wiki/spec/language/state_contracts.md",
                "Every arrival establishes the target's requirements and declared domains under actual argument substitution.",
                "weakened numeric bound in requires",
            )
        return False, "", "", "", "valid: widen did not apply"

    return False, "", "", "", f"valid: unhandled kind {kind}"


def only_q7_mut_violation(spec_quote: str, kind: str) -> bool:
    if kind in ("remove_mut", "amp_to_ampmut"):
        return "mut" in spec_quote.lower() or "writable authority" in spec_quote.lower()
    return False


def triage_row(row: dict[str, str], q7: set[str]) -> dict[str, str]:
    kind = row.get("mutation_kind", "")
    cur = row.get("current_verdict", "")
    rt = row.get("route_verdict", "")
    fixture = row.get("fixture", "")

    if not row.get("route_reason", "").strip():
        row["route_reason"] = extract_route_reason(row.get("route_out", ""), rt)

    out = {
        **row,
        "hand_verdict": "",
        "accepted_by": "",
        "verdict_reason": "",
        "validity_note": "",
        "obligation_line": "",
        "spec_file": "",
        "spec_quote": "",
        "q7_listed": "",
    }

    if kind == "none":
        out["hand_verdict"] = "no-mutation"
        out["verdict_reason"] = "no applicable mutator"
        return out
    if cur == "timeout" or rt == "timeout":
        out["hand_verdict"] = "timeout"
        return out
    if not row.get("mutant_path") and kind != "none":
        out["hand_verdict"] = "failed-run"
        return out

    original = (REPO / fixture).read_text(encoding="utf-8")
    mutant_path = row.get("mutant_path", "")
    mutant = (REPO / mutant_path / "main.omg").read_text(encoding="utf-8") if mutant_path else original

    invalid, ob, sf, sq, vnote = read_mutant_validity(kind, original, mutant)
    out["validity_note"] = vnote
    out["obligation_line"] = ob
    out["spec_file"] = sf
    out["spec_quote"] = sq

    acc = accepted_by(cur, rt)
    c_acc, r_acc = current_accepts(cur), route_accepts(rt)

    if invalid:
        if only_q7_mut_violation(sq, kind) and acc:
            out["hand_verdict"] = "known-gap-q7-mut"
            out["accepted_by"] = acc
            in_list = fixture in q7
            out["q7_listed"] = "yes" if in_list else "no"
            out["verdict_reason"] = (
                f"{'In' if in_list else 'Not in'} PR #2638 list; obligation `{ob}`"
            )
            return out
        if acc:
            out["hand_verdict"] = "confirmed-hole"
            out["accepted_by"] = acc
            out["verdict_reason"] = vnote or ob
            return out
        out["hand_verdict"] = "rejected-both"
        out["verdict_reason"] = vnote or "invalid; both checkers reject"
        return out

    # spec-valid mutant
    if c_acc and r_acc:
        out["hand_verdict"] = "discarded-valid"
        out["verdict_reason"] = vnote
        return out
    if cur != rt:
        out["hand_verdict"] = "disagreement-valid"
        out["verdict_reason"] = vnote
        return out
    out["hand_verdict"] = "rejected-both"
    out["verdict_reason"] = vnote or "valid mutant; both reject"
    return out


def rerun_evidence(mutant_path: str) -> tuple[int, str, str]:
    main = REPO / mutant_path / "main.omg"
    r = subprocess.run(
        [str(OMEGA), "--check", str(main)],
        env=ENV,
        capture_output=True,
        text=True,
        timeout=120,
    )
    r2 = subprocess.run([str(ROUTE), str(main)], capture_output=True, text=True, timeout=120)
    return r.returncode, (r.stdout or "")[:500], (r2.stdout or "")[:500]


def write_hand_block(title: str, rows: list[dict[str, str]], bucket_lines: list[str], n: int = 5) -> None:
    bucket_lines += [f"## {title}", ""]
    if not rows:
        bucket_lines.append("_None._\n")
        return
    for row in rows[:n]:
        mp = row.get("mutant_path", "")
        ec, o_out, r_out = rerun_evidence(mp) if mp else (0, "", "")
        bucket_lines += [
            f"### `{row.get('mutation_kind')}` — `{row.get('fixture')}`",
            f"- Bucket: `{row.get('hand_verdict')}`; accepted_by: `{row.get('accepted_by','')}`",
            f"- Validity (read mutant): {row.get('validity_note') or row.get('verdict_reason')}",
            f"- Route hint (not validity): {row.get('route_reason', '')[:280]}",
            f"- Obligation: `{row.get('obligation_line', '')}`",
            f"- Spec ({row.get('spec_file', '')}): > {row.get('spec_quote', '')}",
        ]
        if mp:
            bucket_lines += [
                f"- `omega --check` exit {ec}:",
                "```text",
                o_out.strip()[:350],
                "```",
                "- Route:",
                "```text",
                r_out.strip()[:350],
                "```",
            ]
        bucket_lines.append("")


def main() -> None:
    hdr, rows = read_rows()
    if not rows:
        print("no rows")
        return

    extra = ["accepted_by", "validity_note", "verdict_reason", "q7_listed"]
    for col in extra:
        if col not in hdr:
            hdr.append(col)

    q7 = q7_fixtures()
    buckets: Counter[str] = Counter()
    out_rows: list[dict[str, str]] = []

    for row in rows:
        triaged = triage_row(row, q7)
        buckets[triaged["hand_verdict"]] += 1
        out_rows.append(triaged)

    with TSV.open("w", encoding="utf-8") as f:
        f.write("\t".join(hdr) + "\n")
        for row in out_rows:
            f.write("\t".join(row.get(h, "") for h in hdr) + "\n")

    total = len(rows)
    assert sum(buckets.values()) == total, (dict(buckets), total)

    confirmed = [r for r in out_rows if r["hand_verdict"] == "confirmed-hole"]
    discarded = [r for r in out_rows if r["hand_verdict"] == "discarded-valid"]
    q7_mut = [r for r in out_rows if r["hand_verdict"] == "known-gap-q7-mut"]

    bucket_lines = [
        f"# Soundness mutations — shard {SHARD}/8",
        "",
        f"PIN: `{PIN}`. Triage follows **soundness_fix3** (either-checker holes; validity from reading mutant).",
        "",
        "## Coverage",
        "",
        f"- Fixtures in shard: **{FIXTURE_TOTAL}**",
        f"- TSV rows (mutant attempts): **{total}**",
        f"- Mutants per fixture: **1** prioritized kind (or `none` row)",
        "",
        "## Bucket totals (sum = mutant rows)",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for name in BUCKETS:
        if buckets[name]:
            bucket_lines.append(f"| `{name}` | {buckets[name]} |")
    for name in sorted(buckets.keys()):
        if name not in BUCKETS:
            bucket_lines.append(f"| `{name}` (legacy) | {buckets[name]} |")
    bucket_lines.append(f"| **Total** | **{total}** |")

    bucket_lines += ["", "### Confirmed-hole by mutation kind", ""]
    for k, v in sorted(Counter(r["mutation_kind"] for r in confirmed).items()):
        bucket_lines.append(f"- `{k}`: {v}")

    bucket_lines += ["", "### Confirmed-hole by accepted_by", ""]
    for k, v in sorted(Counter(r["accepted_by"] for r in confirmed).items()):
        bucket_lines.append(f"- `{k}`: {v}")

    q7_in = sum(1 for r in q7_mut if r.get("q7_listed") == "yes")
    q7_out = sum(1 for r in q7_mut if r.get("q7_listed") == "no")
    bucket_lines += [
        "",
        "## `known-gap-q7-mut` vs PR #2638",
        "",
        f"- Listed in PR #2638 Q7 audit: **{q7_in}**",
        f"- Not in list (still same-binding write/`&mut`/`&write`): **{q7_out}**",
        "",
        "## Commands",
        "```bash",
        "python3 .agents/four-jobs-2026-11-12/soundness/run_shard1.py",
        "python3 .agents/four-jobs-2026-11-12/soundness/postprocess_shard1.py",
        "```",
        "",
    ]

    disc_drop = [r for r in discarded if r.get("mutation_kind") == "drop_requires"]
    write_hand_block("Hand verification — confirmed-hole (5)", confirmed, bucket_lines)
    write_hand_block("Hand verification — discarded-valid (5)", discarded[:5] or discarded, bucket_lines)
    write_hand_block("Hand verification — discarded-valid drop_requires (5)", disc_drop, bucket_lines)
    write_hand_block("Hand verification — known-gap-q7-mut (5)", q7_mut, bucket_lines)

    bucket_lines += ["## Confirmed-hole entries", ""]
    for row in confirmed:
        bucket_lines += [
            f"### `{row['mutation_kind']}` — `{row['fixture']}`",
            f"- accepted_by: **{row['accepted_by']}**",
            f"- Mutant: `{row['mutant_path']}`",
            f"- Current: **{row['current_verdict']}**; route: **{row['route_verdict']}**",
            f"- Validity: {row['validity_note']}",
            f"- Route hint: {row.get('route_reason', '')[:200]}",
            f"- Obligation: `{row['obligation_line']}`",
            f"- Spec: > {row['spec_quote']}",
            "",
        ]

    MD.write_text("\n".join(bucket_lines) + "\n", encoding="utf-8")
    (OUT / f"shard-{SHARD}-buckets.json").write_text(json.dumps(dict(buckets), indent=2) + "\n")
    print("total", total, "confirmed", len(confirmed), dict(buckets))


if __name__ == "__main__":
    main()
