#!/usr/bin/env python3
"""Write shard-S.md summary from shard-S.tsv."""

from __future__ import annotations

import subprocess
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
JOB_DIR = Path(__file__).resolve().parent
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 2
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
TSV = JOB_DIR / f"shard-{SHARD}.tsv"
MD = JOB_DIR / f"shard-{SHARD}.md"
HAND = JOB_DIR / f"shard-{SHARD}-hand-check.md"


def main() -> None:
    rows = []
    for line in TSV.read_text().splitlines()[1:]:
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) >= 7:
            rows.append(
                {
                    "fixture": parts[0],
                    "tier": parts[1],
                    "route": parts[2],
                    "current": parts[3],
                    "agreement": parts[4],
                    "refusal": parts[5],
                    "seconds": parts[6],
                }
            )

    route_c = Counter(r["route"] for r in rows)
    agree_c = Counter(r["agreement"] for r in rows)
    judged = route_c["accepted"] + route_c["rejected"]
    outside = route_c["outside"]
    timeouts = route_c["timeout"]
    errors = route_c["error"]

    # Rank constructs that block judgment (outside verdict), including meta buckets.
    refusal_c = Counter(
        r["refusal"] for r in rows if r["route"] == "outside" and r["refusal"]
    )

    disagreements = [r for r in rows if r["agreement"] == "disagree"]
    soundness_top = [
        r
        for r in disagreements
        if r["route"] == "accepted" and r["current"] == "rejected"
    ]
    false_pass = [
        r
        for r in disagreements
        if r["route"] == "rejected" and r["current"] == "accepted"
    ]

    host = subprocess.check_output(["uname", "-a"], text=True).strip()
    harness = REPO / "target/debug/examples/route_one_comparison"

    lines = [
        f"# Route coverage shard {SHARD} (four-jobs 2026-11-12)",
        "",
        f"Measured commit: `{PIN}`",
        "",
        "## Host",
        "",
        f"```\n{host}\n```",
        "",
        "## Harness",
        "",
        "```bash",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \\",
        "  mbx build -p compiler --example route_one_comparison",
        "```",
        "",
        f"Binary: `{harness}`",
        "",
        "## Commands",
        "",
        "```bash",
        "git ls-files 'tests/omega/pass/**/main.omg' 'tests/omega/run/**/main.omg' \\",
        "  'tests/omega/fail/**/main.omg' | LC_ALL=C sort",
        "# shard S = 0-based index i where i % 8 == S",
        "",
        "timeout 300 target/debug/examples/route_one_comparison <fixture>/main.omg < /dev/null",
        "timeout 300 target/debug/examples/route_one_comparison --every-machine \\",
        "  <fail-fixture>/main.omg < /dev/null",
        "",
        f"python3 .agents/four-jobs-2026-11-12/1/run_shard.py {SHARD} 3",
        "```",
        "",
        "## Build slot / parallelism",
        "",
        "`tools/build_slot.md` limits **one heavy job per test VM** (mbx build, corpus",
        "sweep, ledger re-record). This shard run is not a corpus sweep: it executes",
        "prebuilt `route_one_comparison` once per fixture (`timeout 300`), matching",
        "`route_one_diagnostics.md` (three fixture processes at a time, `nice -n 19`).",
        "That is below the VM-heavy lock used for `mbx`/`cargo` builds. A prior",
        "serial smoke run (125 fixtures) matched the same verdict distribution as the",
        "parallel driver before the parser fix.",
        "",
        "## Agreement definition",
        "",
        "Each row compares the shard **aggregate** `route_verdict` (same closure",
        "precedence as `route_one_comparison --tally`: any `rejected` root rejects the",
        "fixture; else `accepted` if any closure accepts even when others are outside)",
        "with the harness **`current=`** field from the same `route_one_comparison` run",
        "(the typed-frontier current checker), not corpus tier or ledger.",
        "",
        "- **agree**: both accepted, or both rejected (current `stopped` pairs with",
        "  route `rejected`).",
        "- **disagree**: judged route vs current differ.",
        "- **n/a**: route `outside`, `timeout`, or `error`.",
        "",
        "## Meta first-refusal buckets",
        "",
        "- `meta:no-program-entry`: Build binds no `ProgramEntry`; compilation root is",
        "  not elaborated (common for `fail/`). With `--every-machine`, machine closures",
        "  are still judged; this meta label is skipped when a machine root exists.",
        "- `meta:typed-frontier-stopped`: current checker `stopped` before typed trees;",
        "  route `unreached`.",
        "",
        "## `installed_arrival_*` fixtures",
        "",
        "Owner-noted slow cases `fail/build/installed_arrival_without_context` and",
        "`installed_arrival_wrong_controller` are **not** in shard 2 (indices 950–951,",
        "`i % 8` is 6 and 7). No `installed_arrival_*` paths in this shard.",
        "",
        "## Shard totals",
        "",
        f"- Fixtures in shard: **{len(rows)}**",
        f"- Judged (accepted + rejected): **{judged}**",
        f"- Outside: **{outside}**",
        f"- Timeout (separate bucket): **{timeouts}**",
        f"- Error: **{errors}**",
        f"- Agree: **{agree_c['agree']}**",
        f"- Disagree: **{agree_c['disagree']}**",
        f"- Agreement n/a: **{agree_c['n-a']}**",
        "",
        f"- Disagree route=accepted current=rejected: **{len(soundness_top)}**",
        f"- Disagree route=rejected current=accepted: **{len(false_pass)}**",
        "",
        "### Route verdict counts",
        "",
    ]
    for key in sorted(route_c.keys()):
        lines.append(f"- `{key}`: {route_c[key]}")
    lines.extend(
        [
            "",
            "### Outside grouped by first refusal construct (top 30)",
            "",
        ]
    )
    for refusal, count in refusal_c.most_common(30):
        examples = [
            r["fixture"]
            for r in rows
            if r["route"] == "outside" and r["refusal"] == refusal
        ][:3]
        ex = ", ".join(f"`{e}`" for e in examples)
        lines.append(f"- **{refusal}** ({count}): {ex}")

    lines.extend(
        [
            "",
            "### Agreement hand-check (10 fixtures)",
            "",
            "Five `disagree` with route accepted / current rejected, five with route",
            "rejected / current accepted. Harness excerpts:",
            "",
        ]
    )
    if HAND.is_file():
        # append body without duplicate title
        hand = HAND.read_text().splitlines()
        if hand and hand[0].startswith("#"):
            hand = hand[1:]
        lines.extend(hand)
    else:
        lines.append("(run `hand_check_agreement.py` to generate excerpts)")

    lines.extend(
        [
            "",
            "### Disagreements (route accepts, current rejects)",
            "",
        ]
    )
    for r in soundness_top[:30]:
        lines.append(f"- `{r['fixture']}` first_refusal=`{r['refusal']}`")
    if len(soundness_top) > 30:
        lines.append(f"- … {len(soundness_top) - 30} more in TSV")

    MD.write_text("\n".join(lines) + "\n")
    print(f"wrote {MD}")
    print("top10 refusals:")
    for refusal, count in refusal_c.most_common(10):
        print(f"  {count}\t{refusal}")


if __name__ == "__main__":
    main()
