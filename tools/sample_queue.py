#!/usr/bin/env python3
"""The sample loop's work queue: failing samples, smallest first.

Reads the latest unfiltered application coverage or explicit partial work discovery
record (from the main checkout's `build/sample-gate-runs/`, shared by every
worktree, or `--record PATH`) and lists measured failing programs on that
run's target, ordered with unclaimed programs first, then by their own source size so work is
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

Partial discovery leaves unmeasured applications unknown. Probes
(`samples/**/probes/`) and invalid controls (`controls/`) are not sample programs;
`--all` requires complete full-gate coverage and includes them after the programs.

    python3 tools/sample_queue.py
    python3 tools/sample_queue.py --limit 5
    python3 tools/sample_queue.py --all --json

Reads the live outcome registry once. Claimed samples stay visible with their
owners and notes. Failed registry reads mark ownership unknown; they never imply
that a sample is unclaimed. Shared implementation paths do not change priority.

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
import claims as outcome_claims  # noqa: E402
import sample_native_gate  # noqa: E402
from sample_projects import work_kind  # noqa: E402

RUNS = Path(sample_native_gate.shared_runs_directory())
OWNER_QUESTION = re.compile(r"OWNER_QUESTIONS\.md")
# Results that mean the sample does not yet run with its authored result.
FAILING = ("no-native-build", "oracle-failed", "error", "timeout", "check-fail",
           "rejection-failed", "discovery-error")


def latest_record(include_all=False):
    """Newest application work evidence, or full coverage for --all.

    Explicit partial discovery records can supply measured failures, while an
    empty discovery never declares the queue exhausted. Ordinary filtered runs
    cannot replace complete coverage. Legacy records are full gate runs.
    """
    candidates = set(glob.glob(str(RUNS / "*.json")))
    candidates.update(glob.glob(str(ROOT / "build" / "sample-gate-runs" / "*.json")))
    for path in sorted(candidates, key=os.path.basename, reverse=True):
        try:
            record = json.loads(Path(path).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        scope = record.get("scope", "all")
        if not record.get("filter") and scope in ("all", "programs", "discovery") \
                and (not include_all or scope == "all"):
            if scope != "discovery" or any(row.get("state") in FAILING
                                            for row in record.get("results", [])):
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


def commits_behind(revision):
    if not revision:
        return None
    completed = subprocess.run(["git", "rev-list", "--count", f"{revision}..HEAD"],
                               cwd=ROOT, capture_output=True, text=True)
    return int(completed.stdout.strip()) if completed.returncode == 0 else None


def live_ownership():
    """One authoritative snapshot; missing coordination is not an empty registry."""
    snapshot = outcome_claims.Claims(ROOT, "origin").snapshot()
    return outcome_claims.live_claims(snapshot["record"])


def claimed_sample(item):
    """Claims name outcomes, not shared implementation fences."""
    return item.replace("\\", "/").removesuffix("/main.omg").rstrip("/")


def queue(record, include_all=False, live_claims=None):
    owners = {}
    if live_claims is not None:
        for claim in outcome_claims.live_claims({"claims": live_claims}):
            owners.setdefault(claimed_sample(claim["item"]), []).append(claim)
    rows = []
    for result in record.get("results", []):
        if result.get("state") not in FAILING:
            continue
        sample_kind = work_kind(result["sample"])
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
            "claims_known": live_claims is not None,
            "claims": owners.get(result["sample"], []),
        })
    order = {"program": 0, "waiting": 1, "no oracle": 2, "probe": 3, "control": 4}
    return sorted(rows, key=lambda row: (order[row["kind"]], bool(row["claims"]), row["lines"], row["sample"]))


def discovery_candidates(samples, record, live_claims):
    """Reuse failure names as leads, never their historical verdicts.

    Claims name an outcome, not shared paths. Missing ownership stays unknown;
    owner decisions and programs without an oracle cannot supply advance work.
    """
    occupied = {claimed_sample(claim["item"]) for claim in
                outcome_claims.live_claims({"claims": live_claims or []})}
    failures = {row["sample"] for row in record.get("results", [])
                if row.get("state") in FAILING}
    available = [sample for sample in samples if sample not in occupied
                 and not owner_question(sample) and has_oracle(sample)]
    return sorted(available, key=lambda sample:
                  (sample not in failures, source_lines(sample), sample))


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--record", help="a sample gate run record; default: the latest")
    parser.add_argument("--all", action="store_true", help="include probes and invalid controls")
    parser.add_argument("--limit", type=int, help="show at most this many rows")
    parser.add_argument("--json", action="store_true", help="print rows as JSON")
    arguments = parser.parse_args()

    path = arguments.record or latest_record(arguments.all)
    if not path:
        command = "python3 tools/sample_native_gate.py" + ("" if arguments.all else " --programs-only --find-work")
        print(f"sample_queue: no gate run recorded; run {command} first",
              file=sys.stderr)
        return 2
    record = json.loads(Path(path).read_text(encoding="utf-8"))
    if arguments.all and record.get("scope", "all") != "all":
        print("sample_queue: --all requires a full gate record including probes and controls",
              file=sys.stderr)
        return 2
    try:
        ownership = live_ownership()
    except outcome_claims.ClaimsError as error:
        ownership = None
        print(f"sample_queue: ownership unknown: {error}; "
              "do not interpret this list as unclaimed work", file=sys.stderr)
    rows = queue(record, arguments.all, ownership)
    has_work = any(row["kind"] == "program" for row in rows)
    has_unclaimed_work = any(row["kind"] == "program" and not row["claims"] for row in rows)
    if arguments.limit is not None:
        rows = rows[:arguments.limit]
    if arguments.json:
        print(json.dumps(rows, indent=1))
        return 0

    behind = commits_behind(record.get("revision"))
    summary = record.get("summary", {})
    print(f"sample_queue: run {os.path.basename(path)} on {record.get('target', '?')}, "
          f"scope {record.get('scope', 'all')}, "
          f"{summary.get('passed', '?')}/{summary.get('scored', '?')} samples meet their oracle"
          + (f", {behind} commits behind HEAD" if behind else ""))
    if behind and behind > 50:
        print("sample_queue: the record is old; rerun python3 tools/sample_native_gate.py --programs-only --find-work",
              file=sys.stderr)
    if record.get("scope") == "discovery":
        print("sample_queue: partial work discovery; unmeasured applications remain unknown")
    if ownership is None:
        print("sample_queue: live ownership unavailable; all rows have unknown ownership")
    if not has_work:
        print("sample_queue: no unblocked application failures in this record")
    elif ownership is not None and not has_unclaimed_work:
        print("sample_queue: all listed application failures have live owners")
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
        for claim in row["claims"]:
            print(f"        active: {claim['owner']} (ticket {claim['ticket']})")
            if claim.get("note"):
                print("        " + " ".join(claim["note"].split()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
