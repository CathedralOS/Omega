#!/usr/bin/env python3
"""The sample loop's work queue: failing samples, smallest first.

Reads the latest unfiltered `tools/sample_native_gate.py` run record (from the
main checkout's `build/sample-gate-runs/`, shared by every worktree, or
`--record PATH`) and lists the sample programs that do not yet run on that
run's target, ordered by their own source size so the next piece of work is
the easiest to trace. Each row carries the gate's refusal. The record's
revision is compared with this checkout's HEAD; a record from an older
revision may list samples that already run, so rerun the gate when it is far
behind.

Only work a session can finish is listed as work. Two kinds of failing program
are listed after it instead:

- waiting: the sample's README cites `OWNER_QUESTIONS.md`; it runs once the
  owner answers, and the README line says which question.
- no oracle: the program states no `Expected exit` or `Expected output
  contains`, so nothing defines it running. Give it an oracle, or move a
  check-only program into `tests/omega/` as a fixture.

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
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import sample_native_gate  # noqa: E402

RUNS = Path(sample_native_gate.shared_runs_directory())
OWNER_QUESTION = re.compile(r"OWNER_QUESTIONS\.md")
# Results that mean the sample does not yet run with its authored result.
FAILING = ("no-native-build", "oracle-failed", "error", "timeout", "check-fail",
           "rejection-failed", "discovery-error")


def latest_record():
    """The newest run that measured every sample. A filtered run is newer but
    partial: taking it would drop every sample outside its filter."""
    candidates = set(glob.glob(str(RUNS / "*.json")))
    candidates.update(glob.glob(str(ROOT / "build" / "sample-gate-runs" / "*.json")))
    for path in sorted(candidates, key=os.path.basename, reverse=True):
        try:
            record = json.loads(Path(path).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if not record.get("filter"):
            return path
    return None


def owner_question(sample):
    """The README line that cites an owner question, or None."""
    readme = ROOT / sample / "README.md"
    if not readme.is_file():
        return None
    for line in readme.read_text(encoding="utf-8", errors="ignore").splitlines():
        if OWNER_QUESTION.search(line):
            return line.strip()
    return None


def has_oracle(sample):
    entry = ROOT / sample / "main.omg"
    if not entry.is_file():
        return True
    exit_code, output = sample_native_gate.oracle(str(ROOT / sample))
    return exit_code is not None or output is not None


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
        refusal = result.get("diagnostic") or result.get("detail", "")
        if sample_kind == "program":
            question = owner_question(result["sample"])
            if question:
                sample_kind, refusal = "waiting", question
            elif not has_oracle(result["sample"]):
                sample_kind = "no oracle"
        if sample_kind in ("probe", "control") and not include_all:
            continue
        rows.append({
            "sample": result["sample"],
            "kind": sample_kind,
            "lines": source_lines(result["sample"]),
            "state": result["state"],
            "refusal": refusal,
        })
    order = {"program": 0, "waiting": 1, "no oracle": 2, "probe": 3, "control": 4}
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
    if not any(row["kind"] == "program" for row in rows):
        print("sample_queue: every sample runs; write a harder one (AGENTS.md, the sample loop)")
    headings = {"waiting": "waiting on an owner decision (not work until answered):",
                "no oracle": "no oracle (give it one, or move a check-only program to tests/omega/):"}
    shown = None
    for row in rows:
        if row["kind"] in headings and row["kind"] != shown:
            print(f"sample_queue: {headings[row['kind']]}")
        shown = row["kind"]
        refusal = " ".join(row["refusal"].split())[:150]
        print(f"{row['lines']:6d}  {row['sample']}  [{row['state']}{'' if row['kind'] == 'program' else ', ' + row['kind']}]")
        print(f"        {refusal}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
