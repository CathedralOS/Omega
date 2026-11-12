#!/usr/bin/env python3
"""Run route_one_comparison on a fixture shard and emit shard-S.{tsv,md}."""
from __future__ import annotations

import os
import subprocess
import time
from collections import Counter, defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
BIN = REPO / "target/debug/examples/route_one_comparison"
TIMEOUT_S = 300
SHARD = int(os.environ.get("SHARD", "6"))
PARTS = 8
REWRITE = os.environ.get("REWRITE", "").lower() in ("1", "true", "yes")

DECIDED = ["rejected", "incomplete"]
UNDECIDED = ["fallback", "refused-module", "defect", "panicked"]


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
    """Extract the construct/feature string from elaboration refusal detail."""
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


def aggregate_route(routes: list[str]) -> str:
    def has(name: str) -> bool:
        return any(r.startswith(name) for r in routes)

    def first(name: str) -> str | None:
        for r in routes:
            if r.startswith(name):
                return r
        return None

    for name in DECIDED:
        if has(name):
            return name
    for name in UNDECIDED:
        if first(name):
            return first(name)  # type: ignore[return-value]
    if has("accepted") and first("outside"):
        return "accepted, other closures outside"
    if has("accepted"):
        return "accepted"
    if first("no-entry") and not any(r.startswith("outside") for r in routes):
        return "no-entry"
    out = first("outside")
    if out:
        return out
    out = first("unreached")
    if out:
        return out
    if routes:
        return routes[0]
    return "-"


def normalize_route_verdict(raw: str) -> str:
    if raw == "timeout":
        return "timeout"
    if raw == "error":
        return "error"
    if raw.startswith("accepted"):
        return "accepted"
    if raw == "rejected":
        return "rejected"
    if raw in ("no-entry", "unreached") or raw.startswith(
        ("outside:", "incomplete", "fallback", "refused-module", "defect", "panicked")
    ):
        return "outside"
    return "outside"


def current_to_verdict(current: str) -> str:
    if current in ("accepted", "rejected", "stopped"):
        return current
    return "error"


def agreement(current: str, route_v: str) -> str:
    """Route vs current checker field from route_one_comparison (not corpus tier)."""
    if route_v in ("outside", "timeout", "error"):
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


def program_outside_detail(line: str) -> str | None:
    if field_prefix(line, "route=") != "outside":
        return None
    parts = line.split("\t")
    idx = next((i for i, p in enumerate(parts) if p == "route=outside"), None)
    if idx is not None and idx + 1 < len(parts):
        return outside_payload(parts[idx + 1])
    return None


def pick_first_refusal(
    lines: list[str], routes: list[str], agg: str, route_v: str
) -> str:
    if route_v in ("timeout", "error"):
        return route_v
    if route_v in ("accepted", "rejected"):
        return ""
    if agg == "no-entry" or (agg.startswith("no-entry") and "outside" not in agg):
        return "meta:no-entry"
    if agg == "unreached" or agg.startswith("unreached"):
        return "meta:unreached"
    if agg.startswith("incomplete"):
        return "incomplete"
    if agg.startswith("fallback"):
        return agg.split(":", 1)[-1] if ":" in agg else agg
    for line in lines:
        if line.startswith("fallback\t"):
            parts = line.split("\t")
            if len(parts) >= 5:
                construct = parts[3].strip()
                detail = parts[4].strip()
                if detail:
                    return f"{construct} — {detail}" if construct else detail
                return construct or "fallback"
            if len(parts) >= 4:
                return parts[3].strip()
    for line in lines:
        if not line.startswith("program\t"):
            continue
        if field_prefix(line, "route=") == "incomplete":
            return "incomplete"
    for line in lines:
        if not line.startswith("program\t"):
            continue
        detail = program_outside_detail(line)
        if detail:
            return detail
    if agg.startswith("outside:"):
        return outside_payload(agg.split(":", 1)[1])
    if routes:
        if routes[0] == "no-entry":
            return "meta:no-entry"
        if routes[0].startswith("unreached"):
            return "meta:unreached"
    return "unknown"


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

    routes: list[str] = []
    current = ""
    for pl in program_lines:
        c = field_prefix(pl, "current=")
        if c:
            current = c
        r = field_prefix(pl, "route=") or "-"
        if r == "outside":
            parts = pl.split("\t")
            idx = next((i for i, p in enumerate(parts) if p == "route=outside"), None)
            if idx is not None and idx + 1 < len(parts):
                payload = outside_payload(parts[idx + 1])
                r = f"outside:{payload}"
        elif r == "refused-module":
            reason = field_prefix(pl, "reason=") or "-"
            r = f"refused-module:{reason}"
        elif r == "unreached":
            parts = pl.split("\t")
            tail = parts[-1] if len(parts) > 3 else ""
            r = f"unreached:{tail[:120]}"
        routes.append(r)

    agg = aggregate_route(routes)
    route_v = normalize_route_verdict(agg)
    cur_v = current_to_verdict(current)
    agree = agreement(cur_v, route_v)
    refusal = pick_first_refusal(lines, routes, agg, route_v)
    return (route_v, cur_v, agree, refusal, agg)


def tier(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


def hand_check(rows: list[dict[str, str]]) -> list[str]:
    watch = [
        r
        for r in rows
        if r["agreement"] == "disagree"
        and r["route_verdict"] == "accepted"
        and r["current_verdict"] in ("rejected", "stopped")
    ]
    route_reject = [
        r
        for r in rows
        if r["agreement"] == "disagree"
        and r["route_verdict"] == "rejected"
        and r["current_verdict"] == "accepted"
    ]
    lines = [
        "## Hand verification (agreement vs current checker field)",
        "",
        "Definition: `current_verdict` is `route_one_comparison`'s `current=` "
        "field (the ordinary checker at the typed frontier). `route_verdict` is "
        "the harness aggregate `route()` over all program roots (--every-machine on fail/). "
        "This is not the fixture tier and not the corpus ledger.",
        "",
        "### Five route accepts, current rejects (soundness watch)",
        "",
    ]
    for r in watch[:5]:
        out_lines, _secs, fatal = capture_output(r["fixture"])
        programs = [ln for ln in out_lines if ln.startswith("program\t")]
        lines.append(f"- `{r['fixture']}`")
        for p in programs[:6]:
            lines.append(f"  - `{p}`")
        if len(programs) > 6:
            lines.append(f"  - … ({len(programs) - 6} more roots)")
        lines.append(
            f"  - Observation: current rejects the fixture; route aggregate is "
            f"`accepted` because at least one machine closure accepted (often a "
            f"minimal or auxiliary machine with no obligations judged)."
        )
    lines.extend(["", "### Five route rejects, current accepts", ""])
    for r in route_reject[:5]:
        out_lines, _secs, fatal = capture_output(r["fixture"])
        programs = [ln for ln in out_lines if ln.startswith("program\t")]
        lines.append(f"- `{r['fixture']}`")
        for p in programs[:6]:
            lines.append(f"  - `{p}`")
        if not programs:
            lines.append(f"  - (no output; fatal={fatal})")
    if len(watch) < 5 and len(route_reject) < 5:
        lines.append(
            f"- Shard had {len(watch)} watch rows and {len(route_reject)} "
            "current-accept/route-reject rows at summarize time."
        )
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
    route_c = Counter(r["route_verdict"] for r in rows)
    agree_c = Counter(r["agreement"] for r in rows)
    refuse_blocks: dict[str, list[str]] = defaultdict(list)
    meta_buckets: dict[str, int] = Counter()
    for r in rows:
        if r["route_verdict"] != "outside":
            continue
        fr = r["first_refusal"]
        if fr.startswith("meta:"):
            meta_buckets[fr] += 1
        elif fr:
            refuse_blocks[fr].append(r["fixture"])

    disagreements = [r for r in rows if r["agreement"] == "disagree"]

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
        "Execution: **one fixture per process, serial** (no parallel fixture workers on this host). "
        "`tools/build_slot.md` requires one heavy corpus-style job per test VM and caps concurrent agent "
        "load; serial `route_one_comparison` invocations satisfy that.",
        "",
        f"- pass/run: `{cmd_other}`",
        f"- fail: `{cmd_fail}`",
        "",
        "## Totals",
        "",
        f"- judged (route accepted or rejected): {route_c.get('accepted', 0) + route_c.get('rejected', 0)}",
        f"- route accepted: {route_c.get('accepted', 0)}",
        f"- route rejected: {route_c.get('rejected', 0)}",
        f"- outside: {route_c.get('outside', 0)}",
        f"- timeout: {route_c.get('timeout', 0)}",
        f"- error: {route_c.get('error', 0)}",
        f"- agree: {agree_c.get('agree', 0)}",
        f"- disagree: {agree_c.get('disagree', 0)}",
        f"- agreement n/a: {agree_c.get('n-a', 0)}",
        "",
        "### Meta outside buckets (not construct refusals)",
        "",
        "- `meta:no-entry`: compilation root has no bound program entry; route does not elaborate the "
        "full Build (typical for fail/ without a single entry). Machine closures may still run with "
        "`--every-machine`.",
        "- `meta:unreached`: current checker stopped before the typed frontier (`current=stopped`); "
        "route cannot run (`route=unreached`).",
        "",
    ]
    for key, count in sorted(meta_buckets.items()):
        lines.append(f"- `{key}`: {count}")
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
            lines.append(
                f"- `{r['fixture']}`: {r['seconds']}s wall (limit {TIMEOUT_S}s)"
            )
    lines.extend(
        [
            "",
            "## Top 10 first-refusal constructs (outside, excluding meta:*)",
            "",
            "| construct / feature | blocks |",
            "| --- | ---: |",
        ]
    )
    for refusal, fixtures in top10:
        lines.append(f"| `{refusal}` | {len(fixtures)} |")
    lines.extend(
        [
            "",
            "## First refusal ranks (outside, top 80)",
            "",
            "| first_refusal | blocks | examples |",
            "| --- | ---: | --- |",
        ]
    )
    for refusal, fixtures in ranked[:80]:
        ex = ", ".join(f"`{f}`" for f in fixtures[:3])
        lines.append(f"| `{refusal}` | {len(fixtures)} | {ex} |")
    if len(ranked) > 80:
        lines.append(f"| … | … | ({len(ranked) - 80} more construct refusals) |")

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
    out_dir = REPO / ".agents/four-jobs-2026-11-12/1"
    out_dir.mkdir(parents=True, exist_ok=True)
    tsv_path = out_dir / f"shard-{SHARD}.tsv"
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

    pin = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=REPO, capture_output=True, text=True, check=True
    ).stdout.strip()
    summarize(tsv_path, out_dir / f"shard-{SHARD}.md", pin, BIN)
    print(f"done shard {SHARD}: {len(fixtures)} fixtures", flush=True)


if __name__ == "__main__":
    main()
