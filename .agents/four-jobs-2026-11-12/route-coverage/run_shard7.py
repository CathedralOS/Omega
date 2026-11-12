#!/usr/bin/env python3
"""Run route_one_comparison on shard 7; write shard-7.tsv/md (resumable)."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HARNESS = REPO / "target/debug/examples/route_one_comparison"
SHARD = 7
TIMEOUT_S = 300
NEAR_TIMEOUT_S = 290.0
OUT_DIR = Path(__file__).resolve().parent
OUT_TSV = OUT_DIR / f"shard-{SHARD}.tsv"
FIXTURE_LIST = OUT_DIR / f"shard-{SHARD}-fixtures.txt"

META_NO_ENTRY = "meta:no-bound-entry"
META_UNREACHED = "meta:stopped-before-typed-frontier"

JUDGED = frozenset({"accepted", "rejected"})
UNJUDGED = frozenset(
    {
        "outside",
        "incomplete",
        "fallback",
        "refused-module",
        "defect",
        "panicked",
        "no-entry",
        "unreached",
        "-",
    }
)

TSV_HEADER = (
    "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\t"
    "first_refusal\tseconds\tflags\tchecker_diag_class\t"
    "roots\tvacuous_accept_roots\n"
)


def shard_fixtures() -> list[str]:
    cmd = [
        "git",
        "-C",
        str(REPO),
        "ls-files",
        "tests/omega/pass/**/main.omg",
        "tests/omega/run/**/main.omg",
        "tests/omega/fail/**/main.omg",
    ]
    raw = subprocess.check_output(cmd, text=True)
    paths = sorted(raw.splitlines(), key=lambda line: line)
    return [p for i, p in enumerate(paths) if i % 8 == SHARD]


def tier(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    return "fail"


def parse_fields(body: str) -> dict[str, str]:
    parts = body.split("\t")
    fields: dict[str, str] = {}
    i = 0
    while i < len(parts):
        part = parts[i]
        if part.startswith("current="):
            fields["current"] = part[len("current=") :]
        elif part.startswith("root="):
            fields["root"] = part[len("root=") :]
        elif part.startswith("route="):
            val = part[len("route=") :]
            if val == "outside" and i + 1 < len(parts):
                next_part = parts[i + 1]
                if not next_part.startswith(
                    ("route_us=", "current=", "root=", "discharge_us=")
                ):
                    fields["route"] = "outside"
                    fields["outside_detail"] = next_part
                    i += 1
                else:
                    fields["route"] = "outside"
            elif val.startswith("refused-module"):
                fields["route"] = "refused-module"
            else:
                fields["route"] = val.split("\t")[0]
        elif part.startswith("reason="):
            fields["reason"] = part[len("reason=") :]
        elif part.startswith("obligations="):
            fields["obligations"] = part[len("obligations=") :]
        elif part.startswith("judged="):
            fields["judged"] = part[len("judged=") :]
        i += 1
    return fields


def program_route_verdict(routes: list[str]) -> str:
    """
    Accepted only if every root is judged and accepted.
    Rejected if any judged root is rejected.
    Partial if some accepted/rejected mix with unjudged roots (no rejection).
    """
    if not routes:
        return "error"
    rejected = any(r == "rejected" for r in routes)
    accepted = any(r == "accepted" for r in routes)
    unjudged = any(r in UNJUDGED for r in routes)
    if rejected:
        return "rejected"
    if accepted and unjudged:
        return "partial"
    if accepted and not unjudged:
        return "accepted"
    if unjudged:
        return "outside"
    return "error"


def refusal_detail_to_construct(detail: str) -> str:
    detail = detail.strip()
    if not detail:
        return ""
    if ": " in detail:
        reason, rest = detail.split(": ", 1)
        if reason == "Unsupported":
            return rest.strip()
        if reason in ("ForeignIdentity", "Unestablished"):
            return f"{reason}: {rest.strip()}"
        return rest.strip() or reason
    return detail


def construct_from_fallback(parts: list[str]) -> str:
    if len(parts) < 4:
        return ""
    construct = parts[3].strip()
    detail = parts[4].strip() if len(parts) > 4 else ""
    if detail and detail not in construct:
        return f"{construct} ({detail})"
    return construct


def construct_from_program_line(line: str) -> str:
    body = "\t".join(line.split("\t")[2:])
    f = parse_fields(body)
    if f.get("route") == "outside" and f.get("outside_detail"):
        return refusal_detail_to_construct(f["outside_detail"])
    if f.get("route") == "refused-module" and f.get("reason"):
        return f"refused-module: {f['reason']}"
    return ""


def first_refusal_from_output(
    lines: list[str], routes: list[str], route_verdict: str
) -> str:
    program_lines = [ln for ln in lines if ln.startswith("program\t")]
    if route_verdict == "partial":
        for line in program_lines:
            body = "\t".join(line.split("\t")[2:])
            f = parse_fields(body)
            if f.get("route") in UNJUDGED and f.get("route") != "no-entry":
                c = construct_from_program_line(line)
                if c:
                    return c
        for line in program_lines:
            if "route=no-entry" in line:
                return META_NO_ENTRY
    for line in lines:
        if line.startswith("fallback\t"):
            c = construct_from_fallback(line.split("\t"))
            if c:
                return c
    for line in program_lines:
        f = parse_fields("\t".join(line.split("\t")[2:]))
        if f.get("route") in UNJUDGED:
            c = construct_from_program_line(line)
            if c:
                return c
    if routes and all(r == "unreached" for r in routes):
        return META_UNREACHED
    if routes and all(r == "no-entry" for r in routes):
        return META_NO_ENTRY
    return ""


def agreement_label(current: str, route_verdict: str) -> str:
    """Only accepted/rejected route rows; current= is typed-frontier checker."""
    if route_verdict not in ("accepted", "rejected"):
        return "n-a"
    if current not in ("accepted", "rejected"):
        return "n-a"
    if current == route_verdict:
        return "agree"
    return "disagree"


def diagnostic_class(message: str) -> str:
    m = message.lower()
    if "program entry" in m or "programentry" in m or "build" in m and "entry" in m:
        return "program-entry/build"
    if "borrow" in m or "loan" in m or "mutable" in m and "reference" in m:
        return "borrow/lifetime"
    if "requires" in m or "required fact" in m or "obligation" in m:
        return "requirement/fact"
    if "overflow" in m or "division" in m or "exact arithmetic" in m or "divide" in m:
        return "arithmetic-safety"
    if "machine `" in m or "state `" in m:
        return "machine-body/effect"
    if "type" in m or "expected" in m and "but" in m:
        return "type/name"
    if "guard" in m or "index" in m or "bound" in m:
        return "bound/index"
    if "trait" in m or "implementation" in m:
        return "trait/impl"
    if "module" in m or "import" in m or "package" in m:
        return "module/package"
    return "other"


def first_checker_diagnostic(lines: list[str]) -> tuple[str, str]:
    """First current-diagnostic line (checker, not route diagnostic)."""
    for line in lines:
        if not line.startswith("current-diagnostic\t"):
            continue
        parts = line.split("\t")
        if len(parts) >= 4:
            msg = parts[3].strip()
            return diagnostic_class(msg), msg[:160]
    return "", ""


def program_lines_fields(lines: list[str]) -> list[dict[str, str]]:
    out: list[dict[str, str]] = []
    for line in lines:
        if not line.startswith("program\t"):
            continue
        body = "\t".join(line.split("\t")[2:])
        out.append(parse_fields(body))
    return out


def parse_program_lines(lines: list[str]) -> tuple[str, list[str]]:
    fields = program_lines_fields(lines)
    current = ""
    routes: list[str] = []
    for f in fields:
        if f.get("current"):
            current = f["current"]
        routes.append(f.get("route", "-"))
    return current, routes


def format_roots(fields: list[dict[str, str]]) -> str:
    parts = []
    for f in fields:
        root = f.get("root", "?")
        route = f.get("route", "-")
        parts.append(f"{root}:{route}")
    return ";".join(parts)


def format_vacuous_accept_roots(fields: list[dict[str, str]]) -> str:
    vacuous: list[str] = []
    for f in fields:
        if f.get("route") != "accepted":
            continue
        try:
            obligations = int(f.get("obligations") or "0")
            judged = int(f.get("judged") or "0")
        except ValueError:
            continue
        if obligations == 0 and judged == 0:
            vacuous.append(f.get("root", "?"))
    return ";".join(vacuous)


def near_timeout_flags(secs: float, route_verdict: str) -> str:
    if route_verdict == "timeout":
        return ""
    if secs >= NEAR_TIMEOUT_S:
        return "near-timeout"
    return ""


def run_one(fixture: str) -> tuple[str, ...]:
    args = [str(HARNESS)]
    if tier(fixture) == "fail":
        args.append("--every-machine")
    args.append(fixture)
    start = time.monotonic()
    try:
        proc = subprocess.run(
            ["timeout", str(TIMEOUT_S), "nice", "-n", "19", *args],
            cwd=REPO,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        )
    except Exception as exc:  # noqa: BLE001
        secs = time.monotonic() - start
        return (
            fixture,
            tier(fixture),
            "error",
            "error",
            "n-a",
            str(exc).replace("\t", " ")[:200],
            f"{secs:.2f}",
            "",
            "",
            "",
            "",
        )
    secs = time.monotonic() - start
    if proc.returncode == 124:
        return (
            fixture,
            tier(fixture),
            "timeout",
            "",
            "n-a",
            "",
            f"{secs:.2f}",
            "",
            "",
            "",
            "",
        )
    if proc.returncode != 0 and not proc.stdout.strip():
        err = (proc.stderr or "nonzero exit").replace("\t", " ")[:200]
        return (
            fixture,
            tier(fixture),
            "error",
            "error",
            "n-a",
            err,
            f"{secs:.2f}",
            "",
            "",
            "",
            "",
        )

    lines = [ln for ln in proc.stdout.splitlines() if ln.strip()]
    if lines == ["panicked"] or (
        len(lines) == 1 and lines[0].endswith("\tpanicked")
    ):
        return (
            fixture,
            tier(fixture),
            "error",
            "error",
            "n-a",
            "panicked",
            f"{secs:.2f}",
            "",
            "",
            "",
            "",
        )

    current, route_by_root = parse_program_lines(lines)
    if not route_by_root:
        return (
            fixture,
            tier(fixture),
            "error",
            current or "error",
            "n-a",
            "no-program-line",
            f"{secs:.2f}",
            "",
            "",
            "",
            "",
        )

    prog_fields = program_lines_fields(lines)
    route_verdict = program_route_verdict(route_by_root)
    refusal = first_refusal_from_output(lines, route_by_root, route_verdict)
    agr = agreement_label(current, route_verdict)
    flags = near_timeout_flags(secs, route_verdict)
    dclass, _ = first_checker_diagnostic(lines)
    roots = format_roots(prog_fields)
    vacuous = format_vacuous_accept_roots(prog_fields)
    return (
        fixture,
        tier(fixture),
        route_verdict,
        current or "-",
        agr,
        refusal.replace("\t", " "),
        f"{secs:.2f}",
        flags,
        dclass,
        roots,
        vacuous,
    )


SOUNDNESS_HANDCHECK = [
    "tests/omega/fail/build/installed_arrival_wrong_controller/main.omg",
    "tests/omega/pass/arithmetic/runtime_transitive_product_exit/main.omg",
    "tests/omega/fail/borrow/lifetime_argument_arity/main.omg",
    "tests/omega/fail/comptime/array_expression_unused_invalid_operands/main.omg",
    "tests/omega/fail/proofs/declaration_call_argument_type/main.omg",
]


def handcheck_soundness_row(
    fixture: str, row: list[str], harness_snippet: str
) -> str:
    """One-line hand-check for route✓current✗ candidate."""
    rv, cur = row[2], row[3]
    if rv == "accepted" and cur == "rejected":
        if "partial" in harness_snippet:
            return "Not a soundness-watch candidate (partial roots in snippet)."
        return (
            "Possible soundness-watch candidate: all roots `route=accepted` "
            "and harness `current=rejected` (typed-frontier checker)."
        )
    if rv == "partial" and cur == "rejected":
        return (
            "Not a soundness-watch candidate: route_verdict=partial "
            "(accepted mixed with unjudged roots)."
        )
    if rv == "rejected" and cur == "accepted":
        return (
            "Stricter route, not soundness watch (route rejects, current accepts)."
        )
    return f"Not in route✓current✗ set (route={rv}, current={cur})."


def snippet_for_fixture(fixture: str) -> str:
    args = [str(HARNESS)]
    if tier(fixture) == "fail":
        args.append("--every-machine")
    args.append(fixture)
    proc = subprocess.run(
        ["timeout", "120", *args],
        cwd=REPO,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
    )
    return "\n".join(
        ln
        for ln in proc.stdout.splitlines()
        if ln.startswith("program\t") or ln.startswith("current-diagnostic\t")
    )


def write_md(fixtures_done: int, *, final: bool = False) -> None:
    md_path = OUT_TSV.with_suffix(".md")
    rows: list[list[str]] = []
    if OUT_TSV.exists():
        with OUT_TSV.open() as f:
            f.readline()
            for line in f:
                rows.append(line.rstrip("\n").split("\t"))

    totals = defaultdict(int)
    outside_groups: dict[str, list[str]] = defaultdict(list)
    disagreements: list[list[str]] = []
    soundness_rows: list[list[str]] = []
    near_timeout: list[list[str]] = []

    for row in rows:
        if len(row) < 11:
            continue
        rv = row[2]
        totals[rv] += 1
        totals["fixtures"] += 1
        if row[4] == "disagree":
            disagreements.append(row)
        if rv == "accepted" and row[3] == "rejected":
            soundness_rows.append(row)
        if rv == "outside" and row[5]:
            if len(outside_groups[row[5]]) < 3:
                outside_groups[row[5]].append(row[0])
        if row[7] == "near-timeout":
            near_timeout.append(row)

    bucket_sum = (
        totals.get("accepted", 0)
        + totals.get("rejected", 0)
        + totals.get("partial", 0)
        + totals.get("outside", 0)
        + totals.get("error", 0)
        + totals.get("timeout", 0)
    )
    agree = sum(1 for r in rows if len(r) > 4 and r[4] == "agree")
    disagree = sum(1 for r in rows if len(r) > 4 and r[4] == "disagree")
    judged_routes = totals.get("accepted", 0) + totals.get("rejected", 0)

    by_tier_class: dict[tuple[str, str], int] = defaultdict(int)
    for row in soundness_rows:
        dclass = row[8] if len(row) > 8 else ""
        by_tier_class[(row[1], dclass or "no-diagnostic")] += 1

    partial_current_rejects: dict[tuple[str, str], int] = defaultdict(int)
    for row in rows:
        if len(row) > 8 and row[2] == "partial" and row[3] == "rejected":
            partial_current_rejects[
                (row[1], row[8] or "no-diagnostic")
            ] += 1

    host = os.uname().nodename
    with md_path.open("w") as out:
        out.write("# Route coverage shard 7 of 8\n\n")
        out.write("Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`\n\n")
        out.write(f"Host: `{host}` (linux x86_64)\n\n")
        out.write("## Commands\n\n")
        out.write("```bash\n")
        out.write("cd /agent/repos/omega\n")
        out.write(
            "CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p compiler "
            "--example route_one_comparison < /dev/null\n"
        )
        out.write(
            "python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard7.py\n"
        )
        out.write(
            "# serial: timeout 300 nice -n 19 target/debug/examples/route_one_comparison "
            "[--every-machine] <fixture>/main.omg < /dev/null\n"
        )
        out.write("```\n\n")
        out.write(
            "**Verdict rules.** `accepted` only if every root is `route=accepted`. "
            "`rejected` if any root is `route=rejected`. "
            "`partial` if some roots accepted and some unjudged (`outside`, "
            "`no-entry`, `unreached`, etc.). "
            "`current_verdict` is harness `current=` from the typed-frontier "
            "package check (`accepted` / `rejected` / `stopped`); "
            "`stopped` is not treated as checker reject for agreement.\n\n"
        )
        out.write(
            f"Fixtures in shard: {fixtures_done} (bucket sum {bucket_sum})\n\n"
        )
        out.write("## Totals (this shard)\n\n")
        out.write("| bucket | count |\n| --- | ---: |\n")
        for key in (
            "accepted",
            "rejected",
            "partial",
            "outside",
            "error",
            "timeout",
        ):
            out.write(f"| {key} | {totals.get(key, 0)} |\n")
        out.write(f"| **sum** | **{bucket_sum}** |\n")
        out.write(f"| agree | {agree} |\n")
        out.write(f"| disagree | {disagree} |\n")
        out.write(
            f"| agree+disagree (= accepted+rejected rows) | "
            f"{agree + disagree} (= {judged_routes}) |\n\n"
        )

        vac_acc_roots = vac_part_roots = 0
        vac_acc_rows = vac_part_rows = 0
        for row in rows:
            if len(row) < 11:
                continue
            vac_list = [v for v in row[10].split(";") if v]
            if not vac_list:
                continue
            if row[2] == "accepted":
                vac_acc_roots += len(vac_list)
                vac_acc_rows += 1
            elif row[2] == "partial":
                vac_part_roots += len(vac_list)
                vac_part_rows += 1
        out.write("## Vacuous accept roots (obligations=0, judged=0)\n\n")
        out.write(
            f"- **accepted** bucket: {vac_acc_roots} roots across "
            f"{vac_acc_rows} fixtures\n"
        )
        out.write(
            f"- **partial** bucket: {vac_part_roots} roots across "
            f"{vac_part_rows} fixtures\n\n"
        )

        if near_timeout:
            out.write("## Near-timeout fixtures (finished under 300s limit)\n\n")
            for row in near_timeout:
                out.write(
                    f"- `{row[0]}` {row[2]} in {row[6]}s "
                    f"(flags={row[7]})\n"
                )
            out.write("\n")

        refusal_counts: dict[str, int] = defaultdict(int)
        for r in rows:
            if len(r) > 5 and r[2] == "outside" and r[5]:
                refusal_counts[r[5]] += 1
        construct_ranked = sorted(
            ((k, refusal_counts[k], outside_groups[k]) for k in refusal_counts),
            key=lambda x: -x[1],
        )
        out.write("## Outside grouped by first refusal (ranked)\n\n")
        out.write("| first refusal | blocks | examples |\n| --- | ---: | --- |\n")
        for refusal, count, examples in construct_ranked[:30]:
            ex = ", ".join(f"`{e}`" for e in examples)
            out.write(f"| {refusal} | {count} | {ex} |\n")
        out.write("\n")

        out.write("## Route accepted, current rejects (soundness watch)\n\n")
        out.write(
            f"Count (route_verdict=accepted, current_verdict=rejected): "
            f"{len(soundness_rows)}\n\n"
        )
        if by_tier_class:
            out.write("| tier | first checker diagnostic class | count |\n")
            out.write("| --- | --- | ---: |\n")
            for (t, c), n in sorted(by_tier_class.items(), key=lambda x: -x[1]):
                out.write(f"| {t} | {c} | {n} |\n")
            out.write("\n")
        elif partial_current_rejects:
            out.write(
                "No rows with `route_verdict=accepted` and `current_verdict=rejected` "
                "under the all-roots-accepted rule. Former partial-accept rows "
                "(machine accepted + unjudged roots) are now `partial` and excluded.\n\n"
            )
            out.write(
                "| tier | checker diagnostic class | count (partial, current rejects) |\n"
            )
            out.write("| --- | --- | ---: |\n")
            for (t, c), n in sorted(
                partial_current_rejects.items(), key=lambda x: -x[1]
            )[:20]:
                out.write(f"| {t} | {c} | {n} |\n")
            out.write("\n")

        if final:
            hand_picks = list(soundness_rows[:5])
            for fix in SOUNDNESS_HANDCHECK:
                if len(hand_picks) >= 5:
                    break
                row = next((r for r in rows if r[0] == fix), None)
                if row and row not in hand_picks:
                    hand_picks.append(row)
            out.write("### Hand-check (five fixtures)\n\n")
            for row in hand_picks[:5]:
                fix = row[0]
                roots = snippet_for_fixture(fix)
                note = handcheck_soundness_row(fix, row, roots)
                dclass = row[8] if len(row) > 8 else ""
                out.write(
                    f"- `{fix}`: route={row[2]}, current={row[3]}, "
                    f"checker class={dclass or 'n/a'}. {note}\n"
                )
        else:
            out.write(
                "### Hand-check (five fixtures)\n\n"
                "(filled when the shard run completes.)\n"
            )

        out.write("\n## Disagreements (accepted/rejected rows only)\n\n")
        for row in disagreements:
            flag = ""
            if row[2] == "accepted" and row[3] == "rejected":
                flag = " (soundness watch)"
            elif row[2] == "rejected" and row[3] == "accepted":
                flag = " (route stricter)"
            out.write(
                f"- `{row[0]}` tier={row[1]} route={row[2]} current={row[3]}{flag}\n"
            )


def load_done() -> set[str]:
    if not OUT_TSV.exists():
        return set()
    done = set()
    with OUT_TSV.open() as f:
        f.readline()
        for line in f:
            if line.strip():
                done.add(line.split("\t", 1)[0])
    return done


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--reset", action="store_true")
    parser.add_argument("--write-md-only", action="store_true")
    args = parser.parse_args()

    if args.write_md_only:
        write_md(len(shard_fixtures()), final=True)
        return 0

    if not HARNESS.is_file():
        print("build harness first", file=sys.stderr)
        return 1

    fixtures = shard_fixtures()
    FIXTURE_LIST.write_text("\n".join(fixtures) + "\n")

    if args.reset:
        OUT_TSV.write_text(TSV_HEADER)

    done = load_done()
    with OUT_TSV.open("a" if OUT_TSV.stat().st_size else "w") as out:
        if OUT_TSV.stat().st_size == 0:
            out.write(TSV_HEADER)
        for i, fixture in enumerate(fixtures):
            if fixture in done:
                continue
            row = run_one(fixture)
            out.write("\t".join(row) + "\n")
            out.flush()
            if (i + 1) % 25 == 0:
                write_md(len(fixtures), final=False)
                print(f"progress {len(done) + i + 1}/{len(fixtures)}", flush=True)

    write_md(len(fixtures), final=True)
    print(f"complete {len(fixtures)} fixtures", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
