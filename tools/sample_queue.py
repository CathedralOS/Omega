#!/usr/bin/env python3
"""The sample loop's work queue: failing samples, smallest first.

Reads the latest `tools/sample_native_gate.py` run record (from
`build/sample-gate-runs/`, or `--record PATH`) and lists the sample programs
that do not yet run on that run's target, ordered by their own source size so
the next piece of work is the easiest to trace. Each row carries the gate's
refusal. The record's revision is compared with this checkout's HEAD; a record
from an older revision may list samples that already run, so rerun the gate
when it is far behind.

Probes (`samples/**/probes/`) and invalid controls (`controls/`) are not
sample programs; `--all` includes them after the programs.

    python3 tools/sample_queue.py
    python3 tools/sample_queue.py --limit 5
    python3 tools/sample_queue.py --all --json

Standard library only; works on Windows, macOS and Linux.
"""

import argparse
import glob
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNS = ROOT / "build" / "sample-gate-runs"
# Results that mean the sample does not yet run with its authored result.
FAILING = ("no-native-build", "oracle-failed", "error", "timeout", "check-fail",
           "rejection-failed", "discovery-error")


def latest_record():
    records = sorted(glob.glob(str(RUNS / "*.json")))
    return records[-1] if records else None


def source_lines(sample):
    """Lines in the sample's own Omega sources, not its nested projects."""
    directory = ROOT / sample
    total = 0
    for path in directory.rglob("*.omg"):
        relative = path.relative_to(directory).parts
        if path.name == "build.omg" or "build" in relative[:-1]:
            continue
        # A nested build root is a separate project with its own place in the queue.
        nested = any((directory.joinpath(*relative[:depth]) / "build.omg").is_file()
                     for depth in range(1, len(relative)))
        if nested:
            continue
        try:
            total += len(path.read_text(encoding="utf-8", errors="ignore").splitlines())
        except OSError:
            pass
    return total


def kind(sample):
    parts = Path(sample).parts
    if "probes" in parts:
        return "probe"
    if "controls" in parts or Path(sample).name.startswith("invalid") \
            or Path(sample).name in ("intended_invalid", "out_of_bounds"):
        return "control"
    return "program"


def commits_behind(revision):
    if not revision:
        return None
    completed = subprocess.run(["git", "rev-list", "--count", f"{revision}..HEAD"],
                               cwd=ROOT, capture_output=True, text=True)
    return int(completed.stdout.strip()) if completed.returncode == 0 else None


def queue(record, include_all=False):
    rows = []
    for result in record.get("results", []):
        if result.get("state") not in FAILING:
            continue
        sample_kind = kind(result["sample"])
        if sample_kind != "program" and not include_all:
            continue
        rows.append({
            "sample": result["sample"],
            "kind": sample_kind,
            "lines": source_lines(result["sample"]),
            "state": result["state"],
            "refusal": result.get("diagnostic") or result.get("detail", ""),
        })
    order = {"program": 0, "probe": 1, "control": 2}
    return sorted(rows, key=lambda row: (order[row["kind"]], row["lines"], row["sample"]))


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--record", help="a sample gate run record; default: the latest")
    parser.add_argument("--all", action="store_true", help="include probes and invalid controls")
    parser.add_argument("--limit", type=int, help="show at most this many rows")
    parser.add_argument("--json", action="store_true", help="print rows as JSON")
    arguments = parser.parse_args()

    path = arguments.record or latest_record()
    if not path:
        print("sample_queue: no gate run recorded; run python3 tools/sample_native_gate.py first",
              file=sys.stderr)
        return 2
    record = json.loads(Path(path).read_text(encoding="utf-8"))
    rows = queue(record, arguments.all)
    if arguments.limit is not None:
        rows = rows[:arguments.limit]
    if arguments.json:
        print(json.dumps(rows, indent=1))
        return 0

    behind = commits_behind(record.get("revision"))
    summary = record.get("summary", {})
    print(f"sample_queue: run {os.path.basename(path)} on {record.get('target', '?')}, "
          f"{summary.get('passed', '?')}/{summary.get('scored', '?')} samples meet their oracle"
          + (f", {behind} commits behind HEAD" if behind else ""))
    if behind and behind > 50:
        print("sample_queue: the record is old; rerun python3 tools/sample_native_gate.py",
              file=sys.stderr)
    if not rows:
        print("sample_queue: every sample runs; write a harder one (AGENTS.md, the sample loop)")
        return 0
    for row in rows:
        refusal = " ".join(row["refusal"].split())[:150]
        print(f"{row['lines']:6d}  {row['sample']}  [{row['state']}{'' if row['kind'] == 'program' else ', ' + row['kind']}]")
        print(f"        {refusal}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
