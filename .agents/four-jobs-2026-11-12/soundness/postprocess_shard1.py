#!/usr/bin/env python3
"""Triage shard-1.tsv per lead review soundness_fix + soundness_fix2."""
from __future__ import annotations

import json
import re
import subprocess
from collections import Counter
from pathlib import Path

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


def ensure_route_reason(row: dict[str, str]) -> str:
    if row.get("route_reason", "").strip():
        return row["route_reason"]
    raw = row.get("route_out", "")
    rt = row.get("route_verdict", "")
    reason = extract_route_reason(raw, rt) if raw else ""
    row["route_reason"] = reason
    return reason


def machine_block_after_requires(source: str) -> str:
    m = re.search(r"^(\s*)requires\b[^\n]*\n", source, re.MULTILINE)
    if not m:
        return ""
    chunk = source[m.end() :]
    end = re.search(r"^\s*machine\s+", chunk, re.MULTILINE)
    return chunk[: end.start()] if end else chunk


def first_requires_predicate(source: str) -> str:
    m = re.search(r"^(\s*)requires\b([^\n]*)\n", source, re.MULTILINE)
    return m.group(2).strip() if m else ""


def requires_duplicated_in_transition(source: str) -> tuple[bool, str, str]:
    req = first_requires_predicate(source)
    if not req:
        return False, "", ""
    if req in source:
        for m in re.finditer(r"transition\s+([^\n{]+)", source):
            guard = m.group(1).strip()
            if req == guard or req in guard or guard in req:
                return True, req, guard
            for frag in re.findall(r"[\w().:]+\s*[=<>!]+\s*[\w().:]+", req):
                if frag.replace(" ", "") in guard.replace(" ", ""):
                    return True, req, guard
    return False, "", ""


def drop_requires_obligation(original: str) -> tuple[bool, str, str, str]:
    block = machine_block_after_requires(original)
    if not block:
        return False, "", "", ""
    for line in block.splitlines():
        stripped = line.strip()
        if stripped.startswith("ensures "):
            return (
                True,
                stripped,
                "wiki/spec/language/state_contracts.md",
                "Every invocation retains both sides of its own contract.",
            )
        if not stripped or stripped in ("{", "}"):
            continue
        if re.search(r"/\s*[^/\s]|/\s*\w|\[\s*\w|embed\(|\.len\b", stripped):
            return (
                True,
                stripped,
                "wiki/spec/language/state_contracts.md",
                "Machine `requires` applies at every invocation, including terminal and recursive calls.",
            )
    return False, "", "", ""


def remove_mut_binding(original: str) -> tuple[str | None, str | None]:
    m = re.search(r"\blet mut (\w+)", original)
    if not m:
        return None, None
    name = m.group(1)
    if re.search(rf"\blet(?:\s+mut)?\s+{re.escape(name)}\b", original[m.end() :]):
        return None, None
    write_line = None
    for line in original.splitlines():
        if re.search(
            rf"(?:&mut\s+{re.escape(name)}\b|&write\s+{re.escape(name)}\b|{re.escape(name)}\s*=(?!=)|{re.escape(name)}\[[^\]]+\]\s*=|{re.escape(name)}\.\w+\s*=)",
            line,
        ):
            write_line = line.strip()
            break
    return name, write_line


def remove_mut_invalid(original: str) -> tuple[bool, str, str, str]:
    name, line = remove_mut_binding(original)
    if not name or not line:
        return False, "", "", ""
    return (
        True,
        f"`{name}`: {line}",
        "wiki/spec/language/ownership.md",
        "modifying an established ordinary local's owned storage requires a `mut` binding.",
    )


def remove_guard_obligation(original: str) -> tuple[bool, str, str, str]:
    m = re.search(r"^\s*transition\s+([^\n]+)\{\s*\n", original, re.MULTILINE)
    if not m:
        return False, "", "", ""
    guard = m.group(1).strip()
    nxt = original[m.end() :]
    first = next((ln.strip() for ln in nxt.splitlines() if ln.strip() and not ln.strip().startswith("machine")), "")
    if first:
        return (
            True,
            f"transition {guard} {{ … {first}",
            "wiki/spec/language/state_contracts.md",
            "Only the selected arm executes, with ordinary once-only operand evaluation.",
        )
    return False, "", "", ""


def amp_to_ampmut_obligation(original: str) -> tuple[bool, str, str, str]:
    for line in original.splitlines():
        if re.search(r"(?<![&])&(?![&\w])", line) and "&mut" not in line:
            return (
                True,
                line.strip(),
                "wiki/spec/language/expressions.md",
                "mutable and write-only lending require explicit `&mut` and `&write` expressions.",
            )
    return False, "", "", ""


def widen_requires_obligation(original: str) -> tuple[bool, str, str, str]:
    m = re.search(r"requires\b[^\n]+", original)
    if m:
        return (
            True,
            m.group(0).strip(),
            "wiki/spec/language/state_contracts.md",
            "Every arrival establishes the target's requirements and declared domains under actual argument substitution.",
        )
    return False, "", "", ""


SPEC_FN = {
    "drop_requires": drop_requires_obligation,
    "remove_mut": remove_mut_invalid,
    "remove_guard_transition": remove_guard_obligation,
    "amp_to_ampmut": amp_to_ampmut_obligation,
    "widen_requires": widen_requires_obligation,
}


def triage_drop_requires_accepted(
    original: str, route: str, route_reason: str
) -> tuple[str, str, str, str, str]:
    """soundness_fix2: per-route-reason bucket for current=accepted drop_requires."""
    unsupported = route in ("outside", "no-entry", "unreached", "unreached") or "Unsupported" in route_reason
    if unsupported:
        return (
            "discarded-valid",
            "",
            "",
            "",
            f"route {route}: {route_reason[:200] or 'unsupported/no-entry'}",
        )
    dup, req_q, tr_q = requires_duplicated_in_transition(original)
    if dup:
        return (
            "discarded-valid",
            "",
            "",
            "",
            f"requires duplicated in transition — requires `{req_q}` vs transition `{tr_q}`",
        )
    invalid, ob, sf, sq = drop_requires_obligation(original)
    if invalid:
        if route in ("rejected", "incomplete") and route_reason:
            return ("confirmed-invalid", ob, sf, sq, f"route proof gap: {route_reason[:220]}")
        if route == "accepted":
            return ("confirmed-invalid", ob, sf, sq, "both checkers accept; obligation remains")
        return (
            "disagree-not-current-accept",
            ob,
            sf,
            sq,
            f"current accepts; route={route}: {route_reason[:180]}",
        )
    return (
        "discarded-valid",
        "",
        "",
        "",
        f"no body/ensures obligation needs dropped requires; route={route}: {route_reason[:180]}",
    )


def triage_row(row: dict[str, str], q7: set[str]) -> tuple[str, str, str, str, str]:
    kind = row["mutation_kind"]
    cur = row["current_verdict"]
    rt = row["route_verdict"]
    fixture = row["fixture"]
    route_reason = ensure_route_reason(row)

    if kind == "none":
        return "discarded-no-mutation", "", "", "", "no applicable mutator"
    if cur == "timeout" or rt == "timeout":
        return "timeout", "", "", "", "checker timeout"
    if row.get("hand_verdict") == "pending-triage" and not row.get("mutant_path") and kind != "none":
        return "failed-run", "", "", "", "mutant staging/check failed"

    original = (REPO / fixture).read_text(encoding="utf-8")

    if kind == "drop_requires" and cur == "accepted":
        return triage_drop_requires_accepted(original, rt, route_reason)

    fn = SPEC_FN.get(kind)
    if not fn:
        return "discarded-no-rule", "", "", "", f"unknown kind {kind}"

    if kind == "drop_requires":
        invalid, ob, sf, sq = fn(original)
    else:
        invalid, ob, sf, sq = fn(original)

    if kind == "remove_mut":
        name, write_line = remove_mut_binding(original)
        if not invalid:
            return "discarded-valid", "", "", "", "remove_mut: no write/&mut on first `let mut` binding"
        if cur == "accepted" and fixture in q7 and name and write_line:
            return (
                "known-gap-q7-mut",
                ob,
                sf,
                sq,
                f"Q7 migration list includes fixture; write line `{write_line}`",
            )

    if not invalid:
        return "discarded-valid", "", "", "", "mutant remains valid under spec"

    if cur != "accepted":
        if cur != rt:
            return "disagree-not-current-accept", ob, sf, sq, f"route={rt}: {route_reason[:160]}"
        return "both-reject", "", "", "", route_reason[:200] or "both rejected"

    return "confirmed-invalid", ob, sf, sq, route_reason[:220] or "current accepts invalid mutant"


def rerun_evidence(mutant_path: str) -> tuple[int, str, str]:
    main = REPO / mutant_path / "main.omg"
    if not main.is_file():
        main = REPO / mutant_path
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
    picked = rows[:n]
    if not picked:
        bucket_lines.append("_None in this shard yet._\n")
        return
    for row in picked:
        mp = row.get("mutant_path", "")
        ec, o_out, r_out = rerun_evidence(mp) if mp else (0, "", "")
        bucket_lines += [
            f"### `{row['mutation_kind']}` — `{row['fixture']}`",
            f"- Bucket: `{row['hand_verdict']}`",
            f"- Verdict reason: {row.get('verdict_reason', '')}",
            f"- Route reason: {row.get('route_reason', '')[:300]}",
            f"- Obligation: `{row.get('obligation_line', '')}`",
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
    for col in ("route_reason", "verdict_reason"):
        if col not in hdr:
            hdr.insert(hdr.index("agree") if "agree" in hdr else len(hdr), col)

    q7 = q7_fixtures()
    buckets: Counter[str] = Counter()
    out_rows: list[dict[str, str]] = []
    confirmed: list[dict[str, str]] = []
    discarded_drop: list[dict[str, str]] = []
    q7_mut: list[dict[str, str]] = []

    for row in rows:
        bucket, ob, sf, sq, reason = triage_row(row, q7)
        row["hand_verdict"] = bucket
        row["verdict_reason"] = reason
        row["obligation_line"] = ob
        row["spec_file"] = sf
        row["spec_quote"] = sq
        buckets[bucket] += 1
        out_rows.append(row)
        if bucket == "confirmed-invalid":
            confirmed.append(row)
        if row["mutation_kind"] == "drop_requires" and bucket == "discarded-valid":
            discarded_drop.append(row)
        if bucket == "known-gap-q7-mut":
            q7_mut.append(row)

    with TSV.open("w", encoding="utf-8") as f:
        f.write("\t".join(hdr) + "\n")
        for row in out_rows:
            f.write("\t".join(row.get(h, "") for h in hdr) + "\n")

    total = len(rows)
    assert sum(buckets.values()) == total, buckets

    prior_mutant_note = (
        "Earlier `shard-1_mutate_run.py` logged ~970 multi-kind mutants (up to 9 per fixture); "
        "this rerun records **one prioritized mutation per fixture** (or `none`), so the TSV row "
        f"count targets **{FIXTURE_TOTAL}** = shard fixture count, not the prior multi-mutant total."
    )

    bucket_lines = [
        f"# Soundness mutations — shard {SHARD}/8",
        "",
        f"PIN: `{PIN}` (`origin/main` at launch).",
        "",
        "## Mutant total accounting",
        "",
        f"- Shard fixtures (`i % 8 == {SHARD}`): **{FIXTURE_TOTAL}**",
        f"- TSV rows (this run): **{total}** (in progress until run finishes)",
        f"- {prior_mutant_note}",
        "",
        "## Bucket totals (each row exactly one bucket; sum = TSV rows)",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for name in sorted(buckets.keys()):
        bucket_lines.append(f"| `{name}` | {buckets[name]} |")
    bucket_lines.append(f"| **Total** | **{total}** |")
    bucket_lines += [
        "",
        "### Confirmed-invalid by mutation kind",
        "",
    ]
    for k, v in sorted(Counter(r["mutation_kind"] for r in confirmed).items()):
        bucket_lines.append(f"- `{k}`: {v}")

    bucket_lines += [
        "",
        "## Commands",
        "```bash",
        f"cd {REPO}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison",
        "OMEGA_LIBRARY_ROOT=$PWD/source/library timeout 120 ./target/debug/omega --check <mutant>/main.omg",
        "timeout 120 ./target/debug/examples/route_one_comparison <mutant>/main.omg",
        "python3 .agents/four-jobs-2026-11-12/soundness/run_shard1.py",
        "python3 .agents/four-jobs-2026-11-12/soundness/postprocess_shard1.py",
        "```",
        "",
    ]

    write_hand_block("Hand verification — confirmed-invalid (5)", confirmed, bucket_lines)
    write_hand_block("Hand verification — discarded-valid `drop_requires` (5)", discarded_drop, bucket_lines)
    write_hand_block("Hand verification — `known-gap-q7-mut` (5)", q7_mut, bucket_lines)

    bucket_lines += ["## Confirmed-invalid holes (current checker accepts)", ""]
    if not confirmed:
        bucket_lines.append("_None after triage (shard may still be running)._")
    else:
        for row in confirmed:
            bucket_lines += [
                f"### `{row['mutation_kind']}` — `{row['fixture']}`",
                f"- Mutant: `{row['mutant_path']}`",
                f"- Current: **{row['current_verdict']}**; route: **{row['route_verdict']}**",
                f"- Route reason: {row.get('route_reason', '')[:280]}",
                f"- Obligation: `{row['obligation_line']}`",
                f"- Spec ({row['spec_file']}): > {row['spec_quote']}",
                "",
            ]

    MD.write_text("\n".join(bucket_lines) + "\n", encoding="utf-8")
    (OUT / f"shard-{SHARD}-buckets.json").write_text(json.dumps(dict(buckets), indent=2) + "\n")
    print("total", total, "confirmed", len(confirmed), dict(buckets))


if __name__ == "__main__":
    main()
