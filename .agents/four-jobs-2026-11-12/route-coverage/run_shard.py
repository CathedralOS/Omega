#!/usr/bin/env python3
"""Run route_one_comparison on a fixture shard and emit shard-S.{tsv,md}."""
from __future__ import annotations

import os
import re
import subprocess
import time
from collections import Counter, defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
BIN = REPO / "target/debug/examples/route_one_comparison"
TIMEOUT_S = 300
SHARD = int(os.environ.get("SHARD", "6"))
PARTS = 8
REWRITE = os.environ.get("REWRITE", "").lower() in ("1", "true", "yes")
MEASURED_PIN = os.environ.get(
    "MEASURED_PIN", "0835b3aaf803ed97f905be42d94727ff58459263"
)


def fixture_list() -> list[str]:
    proc = subprocess.run(
        [
            "git",
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
            "tests/omega/fail/**/main.omg",
        ],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=True,
    )
    paths = sorted(proc.stdout.splitlines(), key=lambda s: s)
    return [p for i, p in enumerate(paths) if i % PARTS == SHARD]


def field_prefix(line: str, prefix: str) -> str | None:
    for part in line.split("\t"):
        if part.startswith(prefix):
            return part[len(prefix) :]
    return None


def outside_payload(detail: str) -> str:
    detail = detail.strip()
    if not detail:
        return "unknown"
    if detail.startswith("Unsupported:"):
        return detail.split(":", 1)[1].strip() or "unsupported construct"
    if ":" in detail:
        head, tail = detail.split(":", 1)
        head, tail = head.strip(), tail.strip()
        if tail:
            return tail
        return head
    return detail


def normalize_root_route(raw_route: str, line: str) -> str:
    """Per-root verdict class for aggregation."""
    if raw_route == "accepted":
        return "accepted"
    if raw_route == "rejected":
        return "rejected"
    if raw_route == "outside":
        parts = line.split("\t")
        idx = next((i for i, p in enumerate(parts) if p == "route=outside"), None)
        if idx is not None and idx + 1 < len(parts):
            return f"outside:{outside_payload(parts[idx + 1])}"
        return "outside"
    if raw_route == "no-entry":
        return "no-entry"
    if raw_route == "unreached":
        return "unreached"
    if raw_route == "incomplete":
        return "incomplete"
    if raw_route == "refused-module":
        reason = field_prefix(line, "reason=") or "-"
        return f"refused-module:{reason}"
    if raw_route in ("defect", "panicked"):
        return raw_route
    if raw_route.startswith("fallback"):
        return "fallback"
    return raw_route or "unknown"


def program_route_verdict(root_classes: list[str]) -> str:
    """Accepted only if every root is accepted; partial if mixed accept + unjudged."""
    if not root_classes:
        return "error"
    judged = {"accepted", "rejected"}
    has_acc = any(c == "accepted" for c in root_classes)
    has_rej = any(c == "rejected" for c in root_classes)
    has_unj = any(c not in judged for c in root_classes)

    if has_acc and has_unj:
        return "partial"
    if has_rej and has_unj:
        return "partial"
    if has_rej:
        return "rejected"
    if has_acc and not has_unj:
        return "accepted"
    return "outside"


def current_to_verdict(current: str) -> str:
    if current in ("accepted", "rejected", "stopped"):
        return current
    return "error"


def agreement(current: str, route_v: str) -> str:
    """Compare route to typed-frontier `current=` only; partial/outside excluded."""
    if route_v in ("outside", "timeout", "error", "partial"):
        return "n-a"
    cur_ok = current == "accepted"
    cur_bad = current in ("rejected", "stopped")
    if route_v == "accepted" and cur_ok:
        return "agree"
    if route_v == "rejected" and cur_bad:
        return "agree"
    if route_v in ("accepted", "rejected"):
        return "disagree"
    return "n-a"


def first_diagnostic_class(lines: list[str]) -> str:
    for line in lines:
        if not line.startswith("current-diagnostic\t"):
            continue
        parts = line.split("\t")
        if len(parts) < 4:
            continue
        msg = parts[3].strip()
        patterns = [
            (r"cannot prove requires contract", "requires contract unproved"),
            (r"does not fit destination", "representability / fit"),
            (r"cannot borrow", "borrow conflict"),
            (r"mutable borrow", "mutable borrow"),
            (r"shared borrow", "shared borrow"),
            (r"after move", "use after move"),
            (r"unproven", "unproven obligation"),
            (r"cannot assign", "assignment"),
            (r"expected", "type mismatch"),
        ]
        low = msg.lower()
        for pat, label in patterns:
            if pat in low:
                return label
        short = msg.split(" in ", 1)[0].strip()
        return short[:80] if short else "unknown"
    return ""


def pick_first_refusal(
    lines: list[str], routes: list[str], route_v: str
) -> str:
    if route_v in ("timeout", "error"):
        return route_v
    if route_v in ("accepted", "rejected"):
        return ""
    if route_v == "partial":
        for line in lines:
            if not line.startswith("program\t"):
                continue
            root = field_prefix(line, "root=") or ""
            raw = field_prefix(line, "route=") or ""
            norm = normalize_root_route(raw, line)
            if norm not in ("accepted", "rejected"):
                if norm.startswith("outside:"):
                    return norm.split(":", 1)[1]
                if norm == "no-entry" and root == "compilation":
                    continue
                if norm == "no-entry":
                    return "meta:no-entry"
                if norm.startswith("unreached"):
                    return "meta:unreached"
                return norm.replace("outside:", "")
        return "partial-mixed"
    if route_v == "outside":
        for line in lines:
            if line.startswith("fallback\t"):
                parts = line.split("\t")
                if len(parts) >= 4:
                    return parts[3].strip()
        for line in lines:
            if not line.startswith("program\t"):
                continue
            raw = field_prefix(line, "route=") or ""
            norm = normalize_root_route(raw, line)
            if norm.startswith("outside:"):
                return norm.split(":", 1)[1]
            if norm == "no-entry":
                return "meta:no-entry"
            if norm.startswith("unreached"):
                return "meta:unreached"
        return "unknown"
    return ""


def capture_output(fixture: str) -> tuple[list[str], float, str | None]:
    args = [str(BIN)]
    if tier(fixture) == "fail":
        args.append("--every-machine")
    args.append(fixture)
    start = time.monotonic()
    try:
        proc = subprocess.run(
            args,
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_S,
        )
    except subprocess.TimeoutExpired:
        return ([], time.monotonic() - start, "timeout")
    seconds = time.monotonic() - start
    if proc.returncode != 0 and not proc.stdout.strip():
        return ([], seconds, "error")
    return ((proc.stdout + proc.stderr).splitlines(), seconds, None)


def analyze_output(
    lines: list[str], seconds: float, fatal: str | None
) -> tuple[str, str, str, str, str]:
    if fatal == "timeout":
        return ("timeout", "", "n-a", "timeout", "timeout")
    if fatal == "error":
        return ("error", "", "n-a", "error", "error")

    program_lines = [ln for ln in lines if ln.startswith("program\t")]
    if not program_lines:
        if any("panicked" in ln for ln in lines):
            return ("outside", "stopped", "n-a", "panicked", "panicked")
        return ("error", "", "n-a", "error", "no-program-line")

    current = ""
    root_classes: list[str] = []
    for pl in program_lines:
        c = field_prefix(pl, "current=")
        if c:
            current = c
        raw = field_prefix(pl, "route=") or "-"
        root_classes.append(normalize_root_route(raw, pl))

    route_v = program_route_verdict(root_classes)
    cur_v = current_to_verdict(current)
    agree = agreement(cur_v, route_v)
    refusal = pick_first_refusal(lines, root_classes, route_v)
    agg = "|".join(root_classes[:8])
    return (route_v, cur_v, agree, refusal, agg)


def tier(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


def assess_soundness_watch(fixture: str, lines: list[str]) -> str:
    """Hand verdict: is route-accept + current-reject a real watch candidate?"""
    program_lines = [ln for ln in lines if ln.startswith("program\t")]
    roots_acc = [field_prefix(p, "root=") for p in program_lines if field_prefix(p, "route=") == "accepted"]
    roots_unj = [
        field_prefix(p, "root=")
        for p in program_lines
        if field_prefix(p, "route=") not in ("accepted", "rejected")
    ]
    if roots_acc and roots_unj:
        return (
            "No — would have been `partial` under corrected rules; "
            f"accepted roots {roots_acc[:3]} coexist with unjudged {roots_unj[:3]}."
        )
    diags = [ln for ln in lines if ln.startswith("current-diagnostic\t")]
    if not diags:
        return "Unclear — current=rejected but no current-diagnostic lines captured."
    msg = diags[0].split("\t", 3)[-1][:120]
    oblig = sum(1 for p in program_lines if "obligations=" in p and not p.endswith("obligations=0"))
    if all("obligations=0" in p or "obligations=0\t" in p for p in program_lines if "accepted" in p):
        return (
            f"No — route accepted with zero obligations judged; checker still rejects "
            f"({msg}…)."
        )
    return f"Possible — all roots accepted; checker rejects at frontier ({msg}…). Review."


def hand_check(rows: list[dict[str, str]]) -> list[str]:
    watch = [
        r
        for r in rows
        if r["agreement"] == "disagree"
        and r["route_verdict"] == "accepted"
        and r["current_verdict"] == "rejected"
    ]
    route_reject = [
        r
        for r in rows
        if r["agreement"] == "disagree"
        and r["route_verdict"] == "rejected"
        and r["current_verdict"] == "accepted"
    ]
    lines = [
        "## Agreement rules",
        "",
        "`current_verdict` is `route_one_comparison`'s typed-frontier checker (`current=`): "
        "`accepted`, `rejected` (checker diagnostics via `current-diagnostic` lines), or "
        "`stopped` (parse/type failure before the typed frontier — not a proof rejection).",
        "",
        "`route_verdict` is **accepted** only when every program root returned `route=accepted`; "
        "**partial** when any root accepted while another root is outside, no-entry, unreached, "
        "incomplete, or otherwise unjudged; **rejected** when every root is accepted or rejected "
        "and at least one rejected.",
        "",
        "Agree/disagree counts apply only to judged rows (`accepted` or `rejected` route verdict).",
        "",
        "### Route accepts, current rejects (soundness watch) by tier × diagnostic",
        "",
    ]
    by_tier_cls: Counter[tuple[str, str]] = Counter()
    for r in watch:
        by_tier_cls[(r["tier"], r.get("diag_class", ""))] += 1
    lines.append("| tier | first diagnostic class | count |")
    lines.append("| --- | --- | ---: |")
    for (t, cls), n in sorted(by_tier_cls.items(), key=lambda x: (-x[1], x[0])):
        lines.append(f"| {t} | `{cls or '-'}` | {n} |")

    lines.extend(["", "### Five soundness-watch hand checks", ""])
    for r in watch[:5]:
        out_lines, _s, fatal = capture_output(fixture := r["fixture"])
        programs = [ln for ln in out_lines if ln.startswith("program\t")]
        lines.append(f"#### `{fixture}`")
        lines.append(f"- Tier: {r['tier']}; diagnostic class: `{r.get('diag_class', '')}`")
        for p in programs[:8]:
            lines.append(f"- `{p}`")
        lines.append(f"- **Candidate?** {assess_soundness_watch(fixture, out_lines)}")
    if len(watch) < 5:
        lines.append(f"- Only {len(watch)} watch rows in this shard after partial correction.")

    lines.extend(["", "### Five route rejects, current accepts", ""])
    for r in route_reject[:5]:
        out_lines, _s, _f = capture_output(r["fixture"])
        for p in [ln for ln in out_lines if ln.startswith("program\t")][:6]:
            lines.append(f"- `{r['fixture']}`: `{p}`")
    return lines


def summarize(tsv_path: Path, md_path: Path, pin: str, bin_path: Path) -> None:
    rows: list[dict[str, str]] = []
    for line in tsv_path.read_text().splitlines():
        if not line or line.startswith("fixture\t"):
            continue
        parts = line.split("\t")
        if len(parts) < 7:
            continue
        rows.append(
            {
                "fixture": parts[0],
                "tier": parts[1],
                "route_verdict": parts[2],
                "current_verdict": parts[3],
                "agreement": parts[4],
                "first_refusal": parts[5],
                "seconds": parts[6],
            }
        )
    for r in rows:
        if r["route_verdict"] == "accepted" and r["current_verdict"] == "rejected":
            out, _, _ = capture_output(r["fixture"])
            r["diag_class"] = first_diagnostic_class(out)

    route_c = Counter(r["route_verdict"] for r in rows)
    agree_c = Counter(r["agreement"] for r in rows)
    refuse_blocks: dict[str, list[str]] = defaultdict(list)
    meta_buckets: Counter[str] = Counter()
    for r in rows:
        if r["route_verdict"] not in ("outside", "partial"):
            continue
        fr = r["first_refusal"]
        if fr.startswith("meta:"):
            meta_buckets[fr] += 1
        elif fr:
            refuse_blocks[fr].append(r["fixture"])

    disagreements = [r for r in rows if r["agreement"] == "disagree"]
    watch_n = sum(
        1
        for r in rows
        if r["route_verdict"] == "accepted" and r["current_verdict"] == "rejected"
    )

    host = subprocess.run(["hostname"], capture_output=True, text=True).stdout.strip()
    cmd_fail = f"timeout {TIMEOUT_S} {bin_path} --every-machine <fixture>"
    cmd_other = f"timeout {TIMEOUT_S} {bin_path} <fixture>"

    ranked = sorted(refuse_blocks.items(), key=lambda kv: (-len(kv[1]), kv[0]))
    top10 = ranked[:10]

    lines = [
        f"# Route coverage shard {SHARD} of {PARTS}",
        "",
        f"Measured commit: `{pin}`",
        f"Host: `{host}`",
        f"Harness: `{bin_path}`",
        "",
        "## Build and run",
        "",
        "```bash",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison < /dev/null",
        "```",
        "",
        f"Shard selection: sorted `main.omg` corpus list with 0-based index `i % {PARTS} == {SHARD}`.",
        f"Fixtures in shard: {len(rows)}.",
        "",
        "Execution: **one fixture per process, serial** (see `tools/build_slot.md`).",
        "",
        f"- pass/run: `{cmd_other}`",
        f"- fail: `{cmd_fail}`",
        "",
        "## Totals",
        "",
        f"- judged (route accepted or rejected): {route_c.get('accepted', 0) + route_c.get('rejected', 0)}",
        f"- route accepted: {route_c.get('accepted', 0)}",
        f"- route rejected: {route_c.get('rejected', 0)}",
        f"- partial: {route_c.get('partial', 0)}",
        f"- outside: {route_c.get('outside', 0)}",
        f"- timeout: {route_c.get('timeout', 0)}",
        f"- error: {route_c.get('error', 0)}",
        f"- agree: {agree_c.get('agree', 0)}",
        f"- disagree: {agree_c.get('disagree', 0)}",
        f"- agreement n/a (outside + partial + timeout + error): {agree_c.get('n-a', 0)}",
        f"- route accepted + current rejected (watch list, not all candidates): {watch_n}",
        "",
    ]
    for key, count in sorted(meta_buckets.items()):
        lines.append(f"- `{key}` (in outside/partial rows): {count}")
    lines.extend(
        [
            "",
            "### Timeouts",
            "",
        ]
    )
    timeout_rows = [r for r in rows if r["route_verdict"] == "timeout"]
    if not timeout_rows:
        lines.append("(none in this shard)")
    else:
        for r in timeout_rows:
            lines.append(f"- `{r['fixture']}`: {r['seconds']}s")
    slow = [
        r
        for r in rows
        if "installed_arrival" in r["fixture"] or float(r["seconds"]) >= 240.0
    ]
    if slow:
        lines.extend(["", "### Slow fixtures", ""])
        for r in slow:
            lines.append(
                f"- `{r['fixture']}`: {r['seconds']}s, route `{r['route_verdict']}`, "
                f"current `{r['current_verdict']}` (completed under {TIMEOUT_S}s cap)."
            )
    lines.extend(
        [
            "",
            "## Top 10 first-refusal constructs (outside/partial)",
            "",
            "| construct / feature | blocks |",
            "| --- | ---: |",
        ]
    )
    for refusal, fixtures in top10:
        lines.append(f"| `{refusal}` | {len(fixtures)} |")

    lines.extend(hand_check(rows))

    lines.extend(["", "## All disagreements", ""])
    if not disagreements:
        lines.append("(none)")
    else:
        lines.append("| fixture | tier | current | route | first_refusal |")
        lines.append("| --- | --- | --- | --- | --- |")
        for r in disagreements:
            lines.append(
                f"| `{r['fixture']}` | {r['tier']} | {r['current_verdict']} | "
                f"{r['route_verdict']} | `{r['first_refusal']}` |"
            )

    md_path.write_text("\n".join(lines) + "\n")


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tsv_path = OUT_DIR / f"shard-{SHARD}.tsv"
    if os.environ.get("SUMMARIZE_ONLY"):
        summarize(tsv_path, OUT_DIR / f"shard-{SHARD}.md", MEASURED_PIN, BIN)
        return

    fixtures = fixture_list()
    header = "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n"

    done: set[str] = set()
    if not REWRITE and tsv_path.exists():
        for line in tsv_path.read_text().splitlines():
            if line and not line.startswith("fixture\t"):
                done.add(line.split("\t", 1)[0])

    mode = "w" if REWRITE or not tsv_path.exists() else "a"
    with tsv_path.open(mode) as out:
        if mode == "w":
            out.write(header)
        for i, fixture in enumerate(fixtures):
            if fixture in done:
                continue
            lines, seconds, fatal = capture_output(fixture)
            rv, cv, agree, refusal, _agg = analyze_output(lines, seconds, fatal)
            out.write(
                f"{fixture}\t{tier(fixture)}\t{rv}\t{cv}\t{agree}\t{refusal}\t{seconds:.3f}\n"
            )
            out.flush()
            if (i + 1) % 25 == 0:
                print(f"[shard {SHARD}] {i + 1}/{len(fixtures)}", flush=True)

    summarize(tsv_path, OUT_DIR / f"shard-{SHARD}.md", MEASURED_PIN, BIN)
    print(f"done shard {SHARD}: {len(fixtures)} fixtures", flush=True)


if __name__ == "__main__":
    main()
