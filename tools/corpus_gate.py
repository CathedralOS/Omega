#!/usr/bin/env python3
"""Corpus outcome gate — the cheap inner-loop regression check.

Builds the `corpus_runner` test target once (a `harness = false` binary that
compiles every `tests/omega/{pass,fail,run}` fixture through the same routes
the canary suite asserts on), runs it, and diffs the emitted per-fixture
outcome records against `tests/omega/corpus_outcomes.txt` (format:
`tools/corpus_records.py`).

The golden records outcome *classes* — `checked`/`rejected`, diagnostic
messages, and whether each fail fixture's expected.txt fragments were
satisfied — so a diff means a fixture's observable behavior moved, not that
an internal assertion's spelling changed. Re-record with `--record` when a
movement is intended (the corpus is not all-green at base; the golden pins
the red as well as the green).

Usage:

    python3 tools/corpus_gate.py            # build runner, run, diff golden
    python3 tools/corpus_gate.py --record   # rebuild + rewrite the golden
    python3 tools/corpus_gate.py --runner target/debug/deps/corpus_runner-*

Subsets — the inner-loop mode:

    python3 tools/corpus_gate.py --filter termination       # one domain
    python3 tools/corpus_gate.py --filter fail/proofs,wire/ # several fragments

Native leg — build every pass and run fixture for this host and execute the
run tier and `*_exit` fixtures, diffing against this host's golden
`tests/omega/corpus_native_<target>.txt`:

    python3 tools/corpus_gate.py --native --filter providers/

Measuring your own change — one pass, not two:

    git stash && python3 tools/corpus_gate.py --baseline --record && git stash pop
    python3 tools/corpus_gate.py --baseline        # after every later edit

`--baseline` keys its file on `git rev-parse HEAD` under ignored
`build/corpus_baselines/`, so it answers "what did I move" even while the
checked-in golden disagrees with this checkout over another lane's in-flight
work. Re-record after moving to a new base.

`--filter` (or `OMEGA_CORPUS_FIXTURE_FILTER` directly) matches comma-separated
trimmed substrings against `tier/group/name` and diffs only the fixtures that
ran, against the same golden. `--shard k/N` (or `OMEGA_CORPUS_SHARD`)
selects a deterministic hash-slice of the corpus — stable across fixture
additions, so shards can be recorded or diffed on separate sessions/machines.
The full corpus is scheduled-workload cost, not loop cost — subsets are the
iteration gate; the unfiltered diff is the baseline/scheduled gate.

This is the iteration smoke gate, not the landing gate: changes to
internal-only surfaces still need the scoped white-box suites named by
AGENTS.md's Validation scope. What it replaces is the habit of running
several per-crate nextest suites just to smell-test an e2e-visible change.

Recording caveat: `timeout` records are hardware- and load-relative — a
fixture near `OMEGA_CORPUS_FIXTURE_SECS` can flap between `timeout` and a
real status across hosts or under different pool contention. Known fixtures
get an adaptive cap of max(cap, 3x golden millis), so boundary flap needs a
genuine slowdown rather than a busier box. `OMEGA_CORPUS_JOBS` overrides the
worker count; record and diff at the same jobs value on a quiet machine —
`millis` inflates uniformly under external load, which floods the perf-move
report. The unfiltered record is the scheduled baseline, not a loop check.
"""

from __future__ import annotations

import argparse

import collections
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import urllib.request
import threading

import corpus_records

ROOT = Path(__file__).resolve().parent.parent
GOLDEN = ROOT / "tests" / "omega" / "corpus_outcomes.txt"

# Omega target names for hosts with a catalogued deployment profile.
HOST_TARGETS = {
    ("windows", "amd64"): "windows_x86_64",
    ("windows", "x86_64"): "windows_x86_64",
    ("linux", "x86_64"): "linux_x86_64",
    ("linux", "aarch64"): "linux_arm64",
    ("darwin", "arm64"): "macos_arm64",
}


def host_target() -> str | None:
    return HOST_TARGETS.get((platform.system().lower(), platform.machine().lower()))


def build_runner() -> Path:
    """Build the runner via cargo/mbx message-format JSON; return its path."""
    tool = "mbx" if shutil_which("mbx") else "cargo"
    args = [tool, "test", "--no-run", "--message-format=json",
            "-p", "compiler", "--test", "corpus_runner"]
    proc = subprocess.run(args, cwd=ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout[-4000:])
        sys.stderr.write(proc.stderr[-4000:])
        raise SystemExit(f"corpus_gate: runner build failed ({proc.returncode})")
    executable = None
    for line in proc.stdout.splitlines():
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if (message.get("reason") == "compiler-artifact"
                and message.get("target", {}).get("name") == "corpus_runner"
                and message.get("executable")):
            executable = message["executable"]
    if executable is None:
        raise SystemExit("corpus_gate: build produced no corpus_runner binary")
    return Path(executable)


def shutil_which(name: str):
    from shutil import which
    return which(name)


def run_runner(binary: Path) -> list[dict]:
    env = dict(os.environ)
    env.pop("NEXTEST", None)
    proc = subprocess.Popen([str(binary)], cwd=ROOT, text=True, env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    buffered_stderr = collections.deque(maxlen=2000)

    def stream_progress():
        for line in proc.stderr:
            if line.startswith("corpus_runner:"):
                sys.stderr.write(line)
                sys.stderr.flush()
            else:
                buffered_stderr.append(line)

    progress = threading.Thread(target=stream_progress, daemon=True)
    progress.start()
    stdout = proc.stdout.read()
    progress.join()
    proc.wait()
    if proc.returncode != 0:
        sys.stderr.write(stdout[-2000:])
        sys.stderr.writelines(list(buffered_stderr)[-200:])
        raise SystemExit(f"corpus_gate: runner exited {proc.returncode}")
    try:
        return corpus_records.parse(stdout)
    except ValueError as error:
        sys.stderr.write(stdout[-2000:])
        sys.stderr.writelines(list(buffered_stderr)[-200:])
        raise SystemExit(f"corpus_gate: runner emitted unreadable records: {error}")


# --- Jev advisory (--jev): per-diff classification + record verdict ---
# Worked example: build/experiments/failure-triage/CLASSIFY.md. Verdict
# reliability 4/4; per-diff labels inside a batch are contaminated by the
# worst member, so low verdicts re-ask each diff alone to localize.

JEV_ENDPOINT = "https://api.typesafe.ai/v1/systemone"
JEV_MODEL = "jev-1.13.0"


def jev_key() -> str:
    key = os.environ.get("TYPESAFE_API_KEY", "").strip()
    if key:
        return key
    try:
        done = subprocess.run(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            capture_output=True, text=True, timeout=15)
        roots = [Path("build"), Path(done.stdout.strip()).parent / "build"]
    except (OSError, subprocess.SubprocessError):
        roots = [Path("build")]
    for root in roots:
        path = root / "typesafe.env.txt"
        if path.is_file():
            keys = [line.partition("=")[2].strip().strip("\"'")
                    for line in path.read_text(encoding="utf-8-sig").splitlines()
                    if line.partition("=")[0].strip() == "TYPESAFE_API_KEY"]
            if len(keys) == 1:
                return keys[0]
    path = Path.home() / ".config" / "typesafe" / "typesafe.env.txt"
    if path.is_file():
        keys = [line.partition("=")[2].strip().strip("\"'")
                for line in path.read_text(encoding="utf-8-sig").splitlines()
                if line.partition("=")[0].strip() == "TYPESAFE_API_KEY"]
        if len(keys) == 1:
            return keys[0]
    return ""


def jev_post(payload: dict, key: str) -> dict:
    request = urllib.request.Request(
        JEV_ENDPOINT, data=json.dumps(payload).encode(),
        headers={"Authorization": f"Bearer {key}",
                 "Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def diff_questions(diffs: list) -> dict:
    questions = {}
    for index in range(len(diffs)):
        questions[f"diff_{index}"] = {
            "type": "choice",
            "instructions": (
                "Classify this fixture outcome change under the commit's "
                "subject and touched files. `intended`: what the commit was "
                "for, a harmless consequence, or only millis moved. "
                "`accidental`: status/expected_satisfied/diagnostics moved "
                "in a way this commit should not produce — pinning it would "
                "record a regression. `needs_review`: undecidable here."),
            "criteria": {
                "intended": "what this commit was for, harmless, or "
                            "timing-only",
                "accidental": "a semantic move this commit should not "
                              "produce — a regression",
                "needs_review": "cannot be decided from this evidence"}}
    questions["record_safe"] = {
        "type": "noul",
        "instructions": (
            "Would re-pinning the golden with these diffs record intended "
            "behavior only (high) or hide a regression (low)?"),
        "criteria": {
            "true": "every diff is intended or harmless; --record is safe",
            "false": "at least one diff looks accidental; --record would "
                     "pin a regression"}}
    return questions


def jev_advise(diffs: list) -> None:
    """Advisory annotation for structural diffs. Never raises; silent when
    no key; OMEGA_JEV_OFFLINE=1 suppresses."""
    if os.environ.get("OMEGA_JEV_OFFLINE", "").strip() == "1":
        return
    if not diffs:
        return
    key = jev_key()
    if not key:
        return
    try:
        subject = subprocess.run(
            ["git", "log", "-1", "--format=%s"], capture_output=True,
            text=True, timeout=10).stdout.strip()
        files = subprocess.run(
            ["git", "show", "--format=", "--name-only", "HEAD"],
            capture_output=True, text=True, timeout=10).stdout.split()[:40]
        state = {"commit_subject": subject,
                 "files_touched": files,
                 "fixture_diffs": diffs[:30]}
        request = {"model": JEV_MODEL, "state": state,
                   "questions": diff_questions(diffs[:30])}
        response = jev_post(request, key)
        answers = response.get("answers", {})
        safe = answers.get("record_safe", {}).get("noul", 0.5)
        verdict = ("looks safe to --record" if safe >= 0.5
                   else "--record would likely pin a regression")
        print(f"  = jev advisory: record_safe {safe:.2f} — {verdict}")
        flagged = [i for i in range(len(diffs[:30]))
                   if answers.get(f"diff_{i}", {}).get("choice")
                   == "accidental"]
        if safe < 0.5 and flagged:
            # Batch labels are contaminated by the worst member, and a solo
            # intended/accidental re-ask conflates "unexplained by this
            # commit" with "intended". Ask the causal question instead:
            # could THIS commit plausibly produce this diff?
            for index in flagged[:10]:
                solo = {"model": JEV_MODEL,
                        "state": {"commit_subject": subject,
                                  "files_touched": files,
                                  "fixture_diffs": [diffs[index]]},
                        "questions": {"caused_by_commit": {
                            "type": "noul",
                            "instructions": (
                                "Could this commit plausibly produce this "
                                "fixture outcome change? Timing-only "
                                "movement is noise (high). A status or "
                                "diagnostic change outside the commit's "
                                "domain is unexplained — unexplained means "
                                "suspicious (low)."),
                            "criteria": {
                                "true": "this commit could plausibly cause "
                                        "this outcome change",
                                "false": "unexplained by this commit — "
                                         "suspicious"}}}}
                try:
                    one = jev_post(solo, key).get("answers", {})
                    caused = one.get("caused_by_commit", {}).get("noul", 0.5)
                except Exception:
                    caused = 0.5
                if caused < 0.5:
                    print(f"    SUSPECT: {diffs[index]['fixture']} "
                          f"(unexplained by commit, {caused:.2f})")
                else:
                    print(f"    probably fine: {diffs[index]['fixture']} "
                          f"({caused:.2f})")
        elif flagged:
            for index in flagged[:10]:
                print(f"    possibly accidental: "
                      f"{diffs[index]['fixture']}")
    except Exception as error:
        print(f"  = jev advisory unavailable: {type(error).__name__}",
              file=sys.stderr)


def shown(value) -> str:
    """One field of a record, as a diff line shows it."""
    if isinstance(value, list):
        return " | ".join(value) if value else "(no diagnostics)"
    if isinstance(value, dict):
        return " ".join(f"{key}:{item}" for key, item in value.items()) or "(no facts)"
    return str(value)


def baseline_path(native: bool = False) -> Path:
    """This checkout's baseline file, named for the commit it was recorded at.

    The checked-in golden pins outcomes for `main` as a whole, so it disagrees
    with any checkout whose base carries another lane's in-flight movement, and
    a single run against it cannot separate that lane's drift from this one's.
    Recording a baseline at the current commit and diffing working-tree edits
    against it answers the question a change actually asks -- what did I move --
    in one corpus pass rather than two.
    """
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=ROOT, capture_output=True, text=True, check=False,
    ).stdout.strip() or "unknown"
    suffix = "-native" if native else ""
    return ROOT / "build" / "corpus_baselines" / f"{revision}{suffix}.txt"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--record", action="store_true",
                        help="rewrite the golden from this run instead of diffing")
    parser.add_argument("--jev", action="store_true",
                        help="annotate structural diffs with an advisory "
                             "intended/accidental classification + "
                             "record_safe verdict (needs TYPESAFE_API_KEY)")
    parser.add_argument("--runner", type=Path,
                        help="use an existing corpus_runner binary instead of building")
    parser.add_argument("--golden", type=Path, default=GOLDEN)
    parser.add_argument("--native", action="store_true",
                        help="build pass and run fixtures for this host, execute "
                             "the run tier and *_exit fixtures, and diff against "
                             "tests/omega/corpus_native_<target>.txt")
    parser.add_argument("--baseline", action="store_true",
                        help="diff against this checkout's own recorded "
                             "outcomes instead of the checked-in golden — "
                             "`--baseline --record` on a clean tree saves "
                             "them, and every later run costs one corpus pass "
                             "instead of stashing and running twice")
    parser.add_argument("--filter",
                        help="comma-separated fixture fragments — run and diff "
                             "only matching fixtures (sets "
                             "OMEGA_CORPUS_FIXTURE_FILTER for the runner)")
    parser.add_argument("--shard",
                        help="k/N — deterministic hash-slice of the corpus "
                             "(sets OMEGA_CORPUS_SHARD for the runner)")
    options = parser.parse_args()
    if options.native:
        target = host_target()
        if target is None:
            print("corpus_gate: this host has no catalogued Omega target for --native")
            return 2
        os.environ["OMEGA_CORPUS_NATIVE"] = "1"
        if options.golden == GOLDEN:
            options.golden = ROOT / "tests" / "omega" / f"corpus_native_{target}.txt"
    if options.baseline:
        if options.golden != GOLDEN and not options.native:
            print("corpus_gate: --baseline and --golden name different goldens")
            return 2
        options.golden = baseline_path(options.native)
        if not options.golden.is_file() and not options.record:
            print(f"corpus_gate: no baseline at {options.golden.relative_to(ROOT)}; "
                  "record one from a clean tree first:\n"
                  "  git stash && python3 tools/corpus_gate.py --baseline --record "
                  "&& git stash pop")
            return 2
        options.golden.parent.mkdir(parents=True, exist_ok=True)

    subset = (bool(options.filter) or bool(options.shard)
              or bool(os.environ.get("OMEGA_CORPUS_FIXTURE_FILTER", "").strip())
              or bool(os.environ.get("OMEGA_CORPUS_SHARD", "").strip()))
    if options.filter:
        os.environ["OMEGA_CORPUS_FIXTURE_FILTER"] = options.filter
    if options.shard:
        os.environ["OMEGA_CORPUS_SHARD"] = options.shard
    # Prior timings widen per-fixture caps for known-slow fixtures
    # (adaptive max(cap, 3x golden millis) — the runner derives them).
    if options.golden.is_file():
        os.environ["OMEGA_CORPUS_TIMINGS"] = str(options.golden)
    binary = options.runner or build_runner()
    records = run_runner(binary)
    rendered = corpus_records.render(records)

    if options.record:
        # A subset record merges into the golden: fixtures that ran get their
        # fresh records, everything else keeps its pin — so sharded/filtered
        # recordings assemble the same file an unfiltered record would.
        if subset and options.golden.is_file():
            prior = {record["fixture"]: record
                     for record in corpus_records.read(options.golden)}
            prior.update({record["fixture"]: record for record in records})
            merged = [prior[fixture] for fixture in sorted(prior)]
            rendered = corpus_records.render(merged)
        elif subset:
            print(f"corpus_gate: warning — no golden at {options.golden}; "
                  f"recording only the {len(records)} selected fixtures",
                  file=sys.stderr)
        # `Path.write_text` grew `newline` in 3.10; this host runs 3.9, and the
        # golden's identity is byte-sensitive, so open the stream directly
        # rather than letting the platform translate the line endings.
        with options.golden.open("w", encoding="utf-8", newline="\n") as stream:
            stream.write(rendered)
        print(f"corpus_gate: recorded {len(records)} fixture outcomes "
              f"to {options.golden}")
        return 0

    if not options.golden.is_file():
        print(f"corpus_gate: no golden at {options.golden}; "
              f"run --record first", file=sys.stderr)
        return 2
    golden_records = corpus_records.read(options.golden)
    golden = {record["fixture"]: record for record in golden_records}
    actual = {record["fixture"]: record for record in records}

    structural_moves = []
    structured_diffs = []
    perf_moves = []
    perf_notes = []
    for fixture, record in actual.items():
        before = golden.get(fixture)
        if before is None:
            structural_moves.append(f"+ {fixture}: new fixture "
                                  f"({record['status']})")
            structured_diffs.append(
                {"fixture": fixture,
                 "before": {"status": "absent"},
                 "after": {"status": record["status"],
                           "expected_satisfied": record["expected_satisfied"],
                           "diagnostics": record["diagnostics"]}})
            continue
        for field in ("status", "expected_satisfied", "facts", "diagnostics"):
            if record[field] != before[field]:
                structural_moves.append(
                    f"~ {fixture}: {field} moved\n"
                    f"    - {shown(before[field])[:300]}\n"
                    f"    + {shown(record[field])[:300]}")
                structured_diffs.append(
                    {"fixture": fixture,
                     "before": {"status": before["status"],
                                "expected_satisfied":
                                    before["expected_satisfied"],
                                "diagnostics": before["diagnostics"]},
                     "after": {"status": record["status"],
                               "expected_satisfied":
                                   record["expected_satisfied"],
                               "diagnostics": record["diagnostics"]}})
                break
        else:
            old_ms, new_ms = before.get("millis", 0), record.get("millis", 0)
            if old_ms >= 2000 and new_ms >= 2 * old_ms and new_ms - old_ms >= 5000:
                perf_moves.append(
                    f"~ {fixture}: {old_ms}ms -> {new_ms}ms (>=2x slowdown)")
            elif old_ms >= 2000 and new_ms <= old_ms // 2 and old_ms - new_ms >= 5000:
                perf_notes.append(
                    f"  {fixture}: {old_ms}ms -> {new_ms}ms (>=2x faster)")
    if not subset:
        for fixture in golden:
            if fixture not in actual:
                structural_moves.append(f"- {fixture}: removed from corpus")
                structured_diffs.append(
                    {"fixture": fixture,
                     "before": {"status": golden[fixture]["status"]},
                     "after": {"status": "absent"}})

    counts = {}
    for record in records:
        counts[record["status"]] = counts.get(record["status"], 0) + 1
    # Two different conditions, reported apart. A fail fixture that rejected
    # without its fragment rejected for the WRONG reason; one that checked did
    # not reject at all, which for the fail tier usually means its expectation
    # names a stage this targetless route never reaches.
    wrong_reason = sum(1 for r in records if r["expected_satisfied"] is False)
    not_rejected = sum(
        1 for r in records if r["tier"] == "fail" and r["status"] == "checked"
    )
    summary = (f"{len(records)} fixtures ({counts}), "
               f"{wrong_reason} rejected without their expected fragment, "
               f"{not_rejected} fail fixtures did not reject")
    executed = [r for r in records if "exit" in r.get("facts", {})]
    if executed:
        exits = collections.Counter(r["facts"]["exit"] for r in executed)
        differs = sum(1 for r in executed if r["facts"].get("stdout") == "differs")
        summary += (f"; executed {len(executed)} (exit codes {dict(exits)}), "
                    f"{differs} with stdout differing from expected")

    if not structural_moves and not perf_moves:
        print(f"corpus_gate: clean — {summary}")
        for line in perf_notes[:20]:
            print(line)
        return 0
    for line in structural_moves[:60]:
        print(line)
    for line in perf_moves[:60]:
        print(line)
    remaining = len(structural_moves) + len(perf_moves) - 120
    if remaining > 0:
        print(f"… {remaining} more moves")
    for line in perf_notes[:20]:
        print(line)
    print(f"corpus_gate: {len(structural_moves)} structural move(s), "
          f"{len(perf_moves)} perf move(s) — {summary}")
    if options.jev:
        jev_advise(structured_diffs)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
