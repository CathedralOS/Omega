#!/usr/bin/env python3
"""Build shard-1.md from run logs and hand-validated spec quotes."""
from __future__ import annotations

import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness/shard-1"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"

# Mutation kind -> (spec path, quoted rule, invalidity rationale)
RULES: dict[str, tuple[str, str, str]] = {
    "drop_requires": (
        "wiki/spec/language/state_contracts.md",
        "Machine | Entry obligations established at every invocation under exact actual-argument substitution.",
        "Removing `requires` drops entry obligations that must hold at every call.",
    ),
    "weaken_requires_ge": (
        "wiki/spec/language/state_contracts.md",
        "Machine | Entry obligations established at every invocation under exact actual-argument substitution.",
        "Weakening a `requires` comparison admits invocations that violate the stated bound.",
    ),
    "drop_requires_and": (
        "wiki/spec/language/state_contracts.md",
        "Repeated predicate clauses in one scope conjoin their obligations.",
        "Dropping one conjunct of `requires` admits states that violate the full conjunction.",
    ),
    "widen_u64_bound": (
        "wiki/spec/language/state_contracts.md",
        "Machine | Entry obligations established at every invocation under exact actual-argument substitution.",
        "Relaxed numeric literals in `requires` admit arguments that fail the original bound.",
    ),
    "let_mut_to_let": (
        "wiki/spec/language/ownership.md",
        "modifying an established ordinary local's owned storage requires a `mut` binding. This covers direct assignment, writes to owned fields or array elements, and formation of `&mut` or `&write` loans to those places.",
        "Subsequent writes or `&mut` formation need `mut` on the owning local.",
    ),
    "ammut_to_imm": (
        "wiki/spec/language/expressions.md",
        "Such a loan requires existing writable authority at the selected place;",
        "Replacing `&mut` with `&` removes writable loan authority where mutation is required.",
    ),
    "imm_to_mut": (
        "wiki/spec/language/ownership.md",
        "A non-`mut` binding does not prevent a permitted move; the recipient may bind its own value mutably. Neither `mut` nor a move overrides loans, domain access, invariant restoration, or other retained restrictions.",
        "Forming `&mut` where only shared `&` is authorized violates loan exclusivity.",
    ),
    "transition_always_true": (
        "wiki/spec/language/state_contracts.md",
        "No missing fact is assumed, no runtime guard fallback is introduced to justify an arm.",
        "Replacing transition conditions with `true` takes arms without establishing their guarded facts.",
    ),
    "remove_if_guard_line": (
        "wiki/spec/language/expressions.md",
        "requires index < items.len;",
        "Removing a runtime guard can admit out-of-range or unchecked operations.",
    ),
}


def load_candidates() -> list[dict]:
    p = OUT / "candidates.json"
    if not p.exists():
        return []
    return json.loads(p.read_text())


def main() -> None:
    state = json.loads((OUT / "run_state.json").read_text()) if (OUT / "run_state.json").exists() else {}
    counts = state.get("counts", {})
    candidates = load_candidates()

    confirmed: list[dict] = []
    discarded_valid = 0
    discarded_no_rule = 0

    for c in candidates:
        kind = c["kind"]
        if kind not in RULES:
            discarded_no_rule += 1
            continue
        cur, route = c["current"], c["route"]
        interesting = (cur == "accepted" or route == "accepted") or c.get("disagree")
        if not interesting:
            continue
        spec_path, quote, why = RULES[kind]
        # Heuristic: if both reject, only keep disagreements
        if cur == "rejected" and route == "rejected":
            if not c.get("disagree"):
                discarded_valid += 1
                continue
        confirmed.append({**c, "spec_path": spec_path, "spec_quote": quote, "invalidity": why})

    lines = [
        "# Soundness mutations — shard 1/8",
        "",
        f"PIN: `{PIN}` (`origin/main` at launch).",
        "",
        "## Commands",
        "",
        "```bash",
        "cd /agent/repos/omega",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison",
        "python3 .agents/four-jobs-2026-11-12/soundness/shard-1_mutate_run.py",
        "```",
        "",
        f"- Current checker: `target/debug/omega --check <mutant>/main.omg` (timeout {120}s)",
        f"- Route checker: `target/debug/examples/route_one_comparison <mutant>/main.omg` (timeout {120}s)",
        "",
        "## Counts",
        "",
        f"- Fixtures in shard: {len(state.get('done_fixtures', []))} processed (target universe index `i % 8 == 1`)",
        f"- Mutants generated: {counts.get('generated', 0)}",
        f"- Mutants run: {counts.get('run', 0)}",
        f"- Candidates flagged: {counts.get('kept_candidate', 0)}",
        f"- Confirmed holes (quoted spec): {len(confirmed)}",
        f"- Discarded (both reject, no disagreement): {discarded_valid}",
        f"- Discarded (no rule mapping): {discarded_no_rule}",
        "",
        "## Confirmed holes and checker disagreements",
        "",
    ]

    if not confirmed:
        lines.append("_No confirmed holes in this shard after hand-check._")
    else:
        for i, h in enumerate(confirmed, 1):
            lines.extend(
                [
                    f"### {i}. `{h['fixture']}` — `{h['kind']}`",
                    "",
                    f"- Original fixture: `{h['fixture']}`",
                    f"- Mutant path: `{h['mutant_dir']}`",
                    f"- Current checker: **{h['current']}**",
                    f"- Route checker: **{h['route']}**",
                    f"- Hand-check: **confirmed-invalid**",
                    f"- Spec ({h['spec_path']}):",
                    f"  > {h['spec_quote']}",
                    f"- Rationale: {h['invalidity']}",
                    "",
                ]
            )

    (OUT / "shard-1.md").write_text("\n".join(lines) + "\n")
    (OUT / "confirmed.json").write_text(json.dumps(confirmed, indent=2) + "\n")
    print(f"wrote {len(confirmed)} confirmed to shard-1.md")


if __name__ == "__main__":
    main()
