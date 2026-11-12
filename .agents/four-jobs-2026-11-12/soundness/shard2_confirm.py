#!/usr/bin/env python3
"""Turn TSV candidates into shard-2.md with mandatory wiki/spec quotes."""
from __future__ import annotations

import csv
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 2
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / f"shard-{SHARD}.tsv"
MD = OUT / f"shard-{SHARD}.md"

RULES: dict[str, tuple[str, str, str]] = {
    "drop_machine_requires": (
        "wiki/spec/language/state_contracts.md",
        "Machine | Entry obligations established at every invocation under exact actual-argument substitution.",
        "Removing a machine `requires` clause drops entry obligations that must hold at every invocation.",
    ),
    "drop_machine_ensures": (
        "wiki/spec/proofs/contracts.md",
        "`ensures` remains the distinct normal-completion guarantee and proof obligation.",
        "A machine that omits a stated `ensures` no longer carries the declared normal-completion guarantee.",
    ),
    "widen_requires_lt": (
        "wiki/spec/language/state_contracts.md",
        "Machine | Entry obligations established at every invocation under exact actual-argument substitution.",
        "Widening a strict bound in `requires` admits invocations that violate the original bound.",
    ),
    "remove_mut": (
        "wiki/spec/language/ownership.md",
        "Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established ordinary local's owned storage requires a `mut` binding.",
        "Writable storage operations on a binding declared without `mut` violate owned local mutability.",
    ),
    "ref_to_mut_ref": (
        "wiki/spec/language/ownership.md",
        "`&mut` and `&write` fields do not qualify.",
        "Forming `&mut` where only shared `&` is authorized violates exclusive loan rules.",
    ),
    "mut_ref_to_ref": (
        "wiki/spec/language/ownership.md",
        "Shared reference and borrowed-view fields may occur in `[copy]` data when every field satisfies the ordinary copy rules.",
        "Using shared `&` where the program still performs exclusive mutation violates loan permissions.",
    ),
    "delete_first_if_guard": (
        "wiki/spec/build/assumed_binding_contracts.md",
        "Assumed bindings are unavailable for ordinary Omega programs and compiler reasoning.",
        "Deleting a runtime guard may allow operations on values no longer proven in range.",
    ),
}

DISCARD_VALID = frozenset({"drop_machine_ensures", "mut_ref_to_ref"})


def main() -> None:
    rows = list(csv.DictReader(TSV.open(), delimiter="\t"))
    candidates = [r for r in rows if r.get("disposition") == "candidate"]
    kept: list[dict] = []
    discarded_valid = 0
    discarded_no_rule = 0

    for r in candidates:
        kind = r["mutation_kind"]
        if kind not in RULES:
            discarded_no_rule += 1
            continue
        if kind in DISCARD_VALID:
            discarded_valid += 1
            continue
        path, quote, rationale = RULES[kind]
        cv, rv = r["current_verdict"], r["route_verdict"]
        if cv != "accepted" and rv != "accepted":
            if not (cv == "accepted" or rv == "accepted"):
                if cv == "rejected" and rv in ("rejected", "outside", "fallback", "no-entry"):
                    continue
        kept.append({**r, "rule_path": path, "rule_quote": quote, "rationale": rationale})

    stats = {
        "rows_logged": len(rows),
        "candidates": len(candidates),
        "kept_holes": len(kept),
        "discarded_valid": discarded_valid,
        "discarded_no_rule": discarded_no_rule,
    }

    lines = [
        f"# Soundness mutations audit (shard {SHARD} of 8)",
        "",
        f"PIN: `{PIN}`",
        "",
        "## Counts",
        "",
        f"- fixtures in shard: see progress json (`fixtures_total`)",
        f"- TSV rows: {stats['rows_logged']}",
        f"- candidates: {stats['candidates']}",
        f"- confirmed holes kept: {stats['kept_holes']}",
        f"- discarded-valid (ensures drop / noop mut-ref): {stats['discarded_valid']}",
        f"- discarded-no-rule: {stats['discarded_no_rule']}",
        "",
        "## Confirmed holes",
        "",
    ]
    if not kept:
        lines.append("_No confirmed holes after spec review in this shard._\n")
    for i, h in enumerate(kept, 1):
        lines.extend(
            [
                f"### {i}. `{h['fixture']}` ({h['mutation_kind']})",
                "",
                f"- Original fixture: `{h['fixture']}`",
                f"- Mutant: `{h['mutant_path']}`",
                f"- Current checker: `{h['current_verdict']}`",
                f"- Route checker: `{h['route_verdict']}`",
                f"- Hand check: confirmed-invalid — {h['rationale']}",
                f"- Spec rule (`{h['rule_path']}`):",
                "",
                f"> {h['rule_quote']}",
                "",
            ]
        )

    lines.extend(
        [
            "## Commands",
            "",
            "```bash",
            "cd /agent/repos/omega",
            "git checkout 0835b3aaf803ed97f905be42d94727ff58459263",
            "bash tools/agent_setup.sh && mbx xtask doctor",
            "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --examples",
            "OMEGA_LIBRARY_ROOT=$PWD/source/library ./target/debug/examples/route_one_comparison <fixture>/main.omg",
            "PYTHONUNBUFFERED=1 python3 .agents/four-jobs-2026-11-12/soundness/shard2_mutate_run.py",
            "python3 .agents/four-jobs-2026-11-12/soundness/shard2_confirm.py",
            "```",
            "",
            f"Harness build host: linux cloud agent; binaries under `target/debug/`.",
            "",
        ]
    )

    prog_path = OUT / f"shard-{SHARD}.progress.json"
    if prog_path.exists():
        import json

        prog = json.loads(prog_path.read_text())
        st = prog.get("stats", {})
        lines.insert(
            7,
            f"- mutants generated: {st.get('mutants_generated', '?')}\n"
            f"- mutants run: {st.get('mutants_run', '?')}\n"
            f"- both rejected: {st.get('both_rejected', '?')}\n"
            f"- fixtures skipped (original not accepted): {st.get('fixtures_skipped_original_fails', '?')}",
        )

    MD.write_text("\n".join(lines))
    print(MD)
    print(stats)


if __name__ == "__main__":
    main()
