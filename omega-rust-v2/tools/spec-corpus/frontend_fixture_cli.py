"""Check each L0 and L1 rejection fixture at both the native and package boundaries."""
from collections import Counter
from pathlib import Path
import re
import subprocess

import handoff_evidence
import reviewed_comparisons
from package_cli_build import ROOT, build


PRODUCERS = {"L0": "lexer", "L1": "parser"}


def expectation(stage, file, source, golden):
    lines = golden.splitlines()
    if len(lines) != (3 if stage == "L0" else 4):
        raise ValueError("invalid golden diagnostic")
    header = re.fullmatch(r"error (\w+) at (\d+):(\d+) bytes (\d+)\.\.(\d+)", lines[0])
    if not header:
        raise ValueError("invalid diagnostic header")
    kind = header[1]
    line, column, start, end = map(int, header.groups()[1:])
    if not 0 <= start <= end <= len(source) or (start == end and end != len(source)):
        raise ValueError("golden range outside source or empty before EOF")
    if (line, column) != (source.count(b"\n", 0, start) + 1, start - source.rfind(b"\n", 0, start)):
        raise ValueError("golden line and byte column disagree with source")
    reason = lines[-1].removeprefix("  ")
    if not reason or lines[-1] != "  " + reason:
        raise ValueError("missing golden reason")
    return kind, reason, f"{file}:{line}:{column}: {PRODUCERS[stage]}: {reason} (bytes {start}..{end})\n"


def command(exe, env, action, name, expected):
    args = [str(exe), "source", action, "--", name]
    run = subprocess.run(args, cwd=ROOT, env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=30)
    return {"action": action, "exit": run.returncode, "stdout": run.stdout.decode("utf8", "backslashreplace"),
            "stderr": run.stderr.decode("utf8", "backslashreplace"),
            "matched": run.returncode == 1 and run.stdout == b"" and run.stderr == expected.encode("utf8")}


def finish(report, directory, environment):
    # Reuse the executable built and identified from Cargo's output during this run.
    build_record = report.get("lowering_cli", {}).get("build") or {}
    ledger = report["frontend_fixture_cli"] = {"build": build_record, "results": []}
    try:
        if build_record:
            if "executable" not in build_record:
                raise ValueError("the package CLI build failed")
            exe = Path(build_record["executable"])
        else:
            exe = build(directory, environment, build_record, "frontend-cli-build.log")
    except (OSError, ValueError) as exc:
        build_record.setdefault("error", str(exc))
        report["measurement_errors"].append(f"frontend fixture CLI build: {exc}")
        exe = None
    actions = ("parse", "lower" if report["last_layer"] == "L5" else "prepare")
    outcomes = reviewed_comparisons.test_results(report)
    receipts = report["handoff_observations"]
    for stage in PRODUCERS:
        crate = Path(handoff_evidence.DOCUMENTS[stage][0]).parent
        fixtures = ROOT / "layers" / crate / "tests/fixtures"
        sources = set(fixtures.glob("*/reject/*.omg"))
        goldens = {f.with_suffix(".omg") for f in fixtures.glob("*/reject/*.expected")}
        if not sources:
            report["measurement_errors"].append(f"{stage} has no rejection fixtures")
        for path in sorted(sources | goldens):
            file = path.relative_to(ROOT).as_posix()
            fixture = path.relative_to(fixtures).as_posix()
            native = handoff_evidence.passing_fixture(stage, fixture, receipts, outcomes)
            row = {"stage": stage, "file": file, "native": native, "commands": [], "verified": False}
            ledger["results"].append(row)
            try:
                row["kind"], row["reason"], expected = expectation(stage, file, path.read_bytes(), path.with_suffix(".expected").read_text(encoding="utf8"))
                row["expected"] = expected
                if exe is None:
                    row["error"] = "package CLI unavailable"
                    continue
                for action in actions:
                    row["commands"].append(command(exe, environment, action, file, expected))
                if not native or not all(check["matched"] for check in row["commands"]):
                    raise ValueError("missing native match or package command disagrees with the golden diagnostic")
                row["verified"] = True
            except (ValueError, OSError, subprocess.TimeoutExpired) as exc:
                row["error"] = str(exc)
                report["measurement_errors"].append(f"{stage} {fixture}: {exc}")


def summary(report):
    if "frontend_fixture_cli" not in report:
        return []
    rows = report["frontend_fixture_cli"]["results"]
    total = Counter(row["stage"] for row in rows)
    matched = Counter(row["stage"] for row in rows if row["verified"])
    last = "lower" if report["last_layer"] == "L5" else "prepare"
    lines = []
    for stage in PRODUCERS:
        reasons = {(row["kind"], row["reason"]) for row in rows if row["stage"] == stage and row["verified"]}
        lines.append(f"{stage} matched {matched[stage]}/{total[stage]} rejection fixtures at the native API and package "
                     f"commands (`parse` and `{last}`). These fixtures verified {len(reasons)} distinct kind and message pairs.")
    return lines
