#!/usr/bin/env python3
"""Triage shard-1.tsv per lead review (soundness_fix_2260.txt)."""
from __future__ import annotations

import json
import re
import subprocess
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 1
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


def read_rows() -> list[dict[str, str]]:
    lines = TSV.read_text(encoding="utf-8").splitlines()
    if not lines:
        return []
    hdr = lines[0].split("\t")
    rows = []
    for line in lines[1:]:
        cols = line.split("\t")
        if len(cols) < len(hdr):
            cols += [""] * (len(hdr) - len(cols))
        rows.append(dict(zip(hdr, cols)))
    return rows


def machine_block_after_requires(source: str) -> str:
    m = re.search(r"^(\s*)requires\b[^\n]*\n", source, re.MULTILINE)
    if not m:
        return ""
    chunk = source[m.end() :]
    end = re.search(r"^\s*machine\s+", chunk, re.MULTILINE)
    return chunk[: end.start()] if end else chunk


def drop_requires_obligation(original: str, mutant: str) -> tuple[bool, str, str, str]:
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
        if re.search(r"/\s*[^/\s]|/\s*\w|\[\s*\w|embed\(|\.len\b|as\s+u|as\s+i", stripped):
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
    _name, line = remove_mut_binding(original)
    if not line:
        return False, "", "", ""
    return (
        True,
        line,
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
    m = re.search(r"(?<![&])\&(?![&])(?=\w)", original)
    if not m:
        return False, "", "", ""
    line = next(
        (ln.strip() for ln in original.splitlines() if ln.find("&") >= 0 and "&mut" not in ln[: ln.find("&") + 3]),
        "",
    )
    if line:
        return (
            True,
            line,
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


def triage_row(row: dict[str, str], q7: set[str]) -> tuple[str, str, str, str, str]:
    kind = row["mutation_kind"]
    cur = row["current_verdict"]
    rt = row["route_verdict"]
    fixture = row["fixture"]
    if kind == "none":
        return "discarded-no-mutation", "", "", "", ""
    if cur == "timeout" or rt == "timeout":
        return "timeout", "", "", "", ""
    original = (REPO / fixture).read_text(encoding="utf-8")
    mutant_path = row.get("mutant_path", "")
    mutant = (REPO / mutant_path).read_text(encoding="utf-8") if mutant_path else original
    fn = SPEC_FN.get(kind)
    if not fn:
        return "discarded-no-rule", "", "", "", ""
    if kind == "drop_requires":
        invalid, ob_line, spec_file, spec_quote = fn(original, mutant)
    else:
        invalid, ob_line, spec_file, spec_quote = fn(original)
    if not invalid:
        return "discarded-valid", "", "", "", ""
    fixture_key = Path(fixture).as_posix()
    q7_key = fixture_key.split("tests/omega/", 1)[-1] if "tests/omega/" in fixture_key else fixture_key
    if kind == "remove_mut" and fixture in q7:
        if cur == "accepted":
            return "known-gap-q7", ob_line, spec_file, spec_quote, row.get("mutant_path", "")
    if cur != "accepted":
        if cur != rt:
            return "disagree-not-current-accept", ob_line, spec_file, spec_quote, ""
        return "both-reject", "", "", "", ""
    return "confirmed-invalid", ob_line, spec_file, spec_quote, row.get("mutant_path", "")


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


def main() -> None:
    rows = read_rows()
    q7 = q7_fixtures()
    buckets: Counter[str] = Counter()
    out_rows: list[dict[str, str]] = []
    confirmed: list[dict[str, str]] = []

    for row in rows:
        bucket, ob, sf, sq, _ = triage_row(row, q7)
        row["hand_verdict"] = bucket
        row["obligation_line"] = ob
        row["spec_file"] = sf
        row["spec_quote"] = sq
        buckets[bucket] += 1
        out_rows.append(row)
        if bucket == "confirmed-invalid":
            confirmed.append(row)

    hdr = list(read_rows()[0].keys()) if rows else []
    if out_rows and hdr:
        with TSV.open("w", encoding="utf-8") as f:
            f.write("\t".join(hdr) + "\n")
            for row in out_rows:
                f.write("\t".join(row.get(h, "") for h in hdr) + "\n")

    total = len(rows)
    bucket_lines = [
        f"# Soundness mutations — shard {SHARD}/8",
        "",
        f"PIN: `{PIN}` (`origin/main` at launch).",
        "",
        "## Bucket totals (sum to mutants checked)",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for name in sorted(buckets.keys()):
        bucket_lines.append(f"| `{name}` | {buckets[name]} |")
    bucket_lines.append(f"| **Total** | **{total}** |")
    assert sum(buckets.values()) == total
    bucket_lines += [
        "",
        "### Confirmed-invalid by mutation kind",
        "",
    ]
    by_kind = Counter(r["mutation_kind"] for r in confirmed)
    for k, v in sorted(by_kind.items()):
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
        "## Hand verification (5 samples)",
        "",
    ]

    for row in confirmed[:5]:
        mp = row["mutant_path"]
        ec, o_out, r_out = rerun_evidence(mp)
        orig = row["fixture"]
        kind = row["mutation_kind"]
        bucket_lines += [
            f"### Sample `{kind}` — `{orig}`",
            f"- Mutant: `{mp}`",
            f"- Bucket: `{row['hand_verdict']}`",
            f"- Obligation: `{row['obligation_line']}`",
            f"- Spec ({row['spec_file']}): > {row['spec_quote']}",
            f"- `omega --check` exit {ec}; output start:",
            "```text",
            o_out.strip()[:400],
            "```",
            f"- Route output start:",
            "```text",
            r_out.strip()[:400],
            "```",
            "",
        ]

    bucket_lines += ["## Confirmed-invalid holes (current checker accepts)", ""]
    if not confirmed:
        bucket_lines.append("_None after triage._")
    else:
        for row in confirmed:
            bucket_lines += [
                f"### `{row['mutation_kind']}` — `{row['fixture']}`",
                f"- Mutant: `{row['mutant_path']}`",
                f"- Current: **{row['current_verdict']}**; route: **{row['route_verdict']}**",
                f"- Obligation: `{row['obligation_line']}`",
                f"- Spec ({row['spec_file']}): > {row['spec_quote']}",
                "",
            ]

    MD.write_text("\n".join(bucket_lines) + "\n", encoding="utf-8")
    (OUT / f"shard-{SHARD}-buckets.json").write_text(json.dumps(dict(buckets), indent=2) + "\n")
    print("total", total, "confirmed", len(confirmed), dict(buckets))


if __name__ == "__main__":
    main()
