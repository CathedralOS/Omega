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
run tier, `*_exit` fixtures and fixtures with `expected_native_exit.txt` (or
a host-specific `expected_native_exit.<target>.txt`, which overrides it),
diffing against this host's golden `tests/omega/corpus_native_<target>.txt`:

    python3 tools/corpus_gate.py --native --filter providers/

An authored `expected_stdout.txt` requires a completed native execution with
`stdout:match`, independently of any exit oracle. These promises are checked
before golden comparison and recording, so `--record` cannot bless wrong or
missing output. A declaration-only or foreign-target build supplies no runtime
evidence and cannot satisfy that promise; use a matching bound host.

`--native-target <target>` builds for another catalogued target and diffs
against that target's golden. Products execute only where the host runs
them: an Apple-silicon macOS host runs `macos_x86_64` under Rosetta
(`arch -x86_64`); other foreign targets are realized without executing.

    python3 tools/corpus_gate.py --native-target macos_x86_64 --filter host/

Interpreted leg — also execute each runnable fixture on the checked
interpreter `omega run --both` uses, diffing against
`tests/omega/corpus_interpreted.txt`; `--both` runs the native and
interpreted legs together, records whether their exit codes agree, and diffs
against `tests/omega/corpus_both_<target>.txt`:

    python3 tools/corpus_gate.py --interpret --filter _exit
    python3 tools/corpus_gate.py --both --filter run/

An authored stdout oracle must match every requested execution engine.
Interpreter output that differs or was not observed rejects before recording,
even when its exit code agrees with native execution.

Reading a move — the summary cuts each field at 300 columns, so use
`--write-moves` to get every moved fixture's before/after records in full:

    python3 tools/corpus_gate.py --filter fail/ --write-moves build/moves.json

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
tools/testing.md's Validation scope. What it replaces is the habit of running
several per-crate nextest suites just to smell-test an e2e-visible change.

Recording caveat: `timeout` records are hardware- and load-relative — a
fixture near `OMEGA_CORPUS_FIXTURE_SECS` can flap between `timeout` and a
real status across hosts or under different pool contention. Known fixtures
get an adaptive cap of max(cap, 3x golden millis), so boundary flap needs a
genuine slowdown rather than a busier box. `OMEGA_CORPUS_JOBS` overrides the
worker count; record and diff at the same jobs value on a quiet machine —
`millis` inflates uniformly under external load, which floods the perf-move
report. `--record` stamps the golden with a `# context:` line (jobs, cpus,
load averages bracketing the run), and a diff that finds perf moves compares
the two contexts and says when one side started on a busy machine; only the
pre-run load counts, since the end sample includes the gate's own workers. The
unfiltered record is the scheduled baseline, not a loop check.

Exit codes — the gate's machine-readable verdict (every run can also write
a full verdict document with `--json PATH`):

    0  clean: no structural or perf moves
    1  moved: structural or perf moves, but no regression
    2  usage error: bad arguments, missing golden, no host target
    3  regressed: at least one fixture verdict is "regressed", or an
       authored exit or stdout oracle failed (also the `--record` refusal)
    4  infrastructure failure: runner build failed, no binary produced,
       runner exited nonzero, or its records were unreadable

`--bisect` remaps for `git bisect run`, whose documented contract is: 125
skips the commit, 1..127 (except 125) mark it bad, 0 marks it good, and
any code above 127 aborts the bisect; so regressed -> 1 (bad), infrastructure
failure -> 125 (skip), everything else -> 0 (good), and usage errors ->
255 (abort, since a usage error says nothing about the commit).
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
    ("darwin", "x86_64"): "macos_x86_64",
}


def host_target() -> str | None:
    return HOST_TARGETS.get((platform.system().lower(), platform.machine().lower()))


NATIVE_EXIT_ORACLE = "expected_native_exit.txt"


def native_exit_oracle(fixture: Path, target: str) -> Path:
    """The fixture's authored exit promise for `target`.

    `expected_native_exit.<target>.txt` overrides the portable
    `expected_native_exit.txt`: a status's physical presentation belongs to
    the target contract (wiki/spec/language/process_exit.md), so one program
    may promise 212 on a Unix host for the semantic status -44 while another
    target keeps no promise at all.
    """
    specific = fixture / f"expected_native_exit.{target}.txt"
    return specific if specific.is_file() else fixture / NATIVE_EXIT_ORACLE


def native_exit_failures(records: list[dict], corpus: Path, target: str) -> list[str]:
    """Check authored runtime oracles before comparing or recording observations.

    A golden may retain an unfinished compile, but cannot redefine an explicit
    runtime promise. `trap` means the ISA crash leaf: BRK/SIGTRAP on ARM64,
    UD2/SIGILL on Unix x86-64 (Linux and Intel macOS), or
    STATUS_ILLEGAL_INSTRUCTION on Windows.
    Segfaults, missing execution and timeouts are not successful traps.
    """
    traps = {"macos_arm64": {"exit": "signal", "signal": "5"},
             "linux_arm64": {"exit": "signal", "signal": "5"},
             "linux_x86_64": {"exit": "signal", "signal": "4"},
             "macos_x86_64": {"exit": "signal", "signal": "4"},
             "windows_x86_64": {"exit": str(-1073741795)}}
    failures = []
    for record in records:
        path = native_exit_oracle(corpus / record["fixture"], target)
        if not path.exists():
            continue
        try:
            expectation = path.read_text(encoding="utf-8").strip()
            if expectation == "trap":
                expected = traps[target]
            else:
                code = int(expectation)
                if not -(2 ** 31) <= code < 2 ** 31:
                    raise ValueError("exit code is outside i32")
                expected = {"exit": str(code)}
        except (OSError, UnicodeError, ValueError, KeyError) as error:
            failures.append(f"{record['fixture']}: invalid native exit expectation ({error})")
            continue
        facts = record.get("facts", {})
        matches = all(facts.get(key) == value for key, value in expected.items())
        if record["status"] != "built" or not matches:
            failures.append(f"{record['fixture']}: expected native {expectation}, "
                            f"observed {record['status']} {facts}")
    return failures


def execution_stdout_failures(records: list[dict], corpus: Path,
                             engine: str) -> list[str]:
    """Enforce authored output for each requested execution engine.

    The runner compares output after normalizing line endings. Require its
    match fact rather than accepting an absent observation: unreadable oracle
    files, skipped execution and compile-only rows cannot establish a match.
    A timed-out process can print all expected bytes before hanging, so output
    also needs completed execution. This does not invent an expected exit code;
    only an authored exit/trap oracle constrains how the process terminates.
    """
    prefix = "interpreter-" if engine == "interpreter" else ""
    accepted_statuses = {"checked", "built"} if prefix else {"built"}
    failures = []
    for record in records:
        if not (corpus / record["fixture"] / "expected_stdout.txt").exists():
            continue
        facts = record.get("facts", {})
        completed = facts.get(prefix + "exit") not in (None, "timeout", "unrunnable")
        if (record["status"] not in accepted_statuses or not completed
                or facts.get(prefix + "stdout") != "match"):
            failures.append(
                f"{record['fixture']}: expected {engine} stdout match from completed "
                f"execution, observed {record['status']} {facts}")
    return failures


class InfrastructureFailure(Exception):
    """The gate could not measure at all — runner build failed, no binary
    was produced, the runner exited nonzero, or its records were
    unreadable. Distinct from a regression: exit 4, or 125 under --bisect
    so `git bisect run` skips the commit instead of blaming it."""


def build_runner() -> Path:
    """Build the runner via cargo/mbx message-format JSON; return its path."""
    tool = "mbx" if shutil_which("mbx") else "cargo"
    args = [tool, "test", "--no-run", "--message-format=json",
            "-p", "compiler", "--test", "corpus_runner"]
    proc = subprocess.run(args, cwd=ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout[-4000:])
        sys.stderr.write(proc.stderr[-4000:])
        raise InfrastructureFailure(
            f"corpus_gate: runner build failed ({proc.returncode})")
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
        raise InfrastructureFailure(
            "corpus_gate: build produced no corpus_runner binary")
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
        raise InfrastructureFailure(
            f"corpus_gate: runner exited {proc.returncode}")
    try:
        return corpus_records.parse(stdout)
    except ValueError as error:
        sys.stderr.write(stdout[-2000:])
        sys.stderr.writelines(list(buffered_stderr)[-200:])
        raise InfrastructureFailure(
            f"corpus_gate: runner emitted unreadable records: {error}")


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


# Status ranks for the pass/run tiers: built means the artifact exists,
# checked means compilation succeeded, anything else is a non-movement
# floor. Fail tiers invert the goal: rejected is success there.
STATUS_RANK = {"built": 3, "checked": 2}


def classify(before: dict | None, after: dict | None) -> tuple[str, list[str]]:
    """One fixture's verdict: (verdict, reasons).

    Verdicts: clean, moved, improved, regressed, perf_slower, perf_faster,
    new, removed. Regressed dominates improved dominates moved; the reasons
    list collects everything that fired. Diagnostics compare normalized
    (corpus_records.normalized_diagnostics) so a diagnostic relabel is not
    a move; recorded_at is measurement provenance, never compared.
    """
    if before is None:
        return "new", ["fixture added"]
    if after is None:
        return "removed", ["fixture removed"]
    regressed, improved, moved = [], [], []
    tier = after["tier"]
    old_status, new_status = before["status"], after["status"]
    if tier == "fail":
        # rejected is the goal here; a fail fixture that stopped rejecting
        # is a soundness regression, never an improvement.
        if old_status == "rejected" and new_status != "rejected":
            regressed.append("fail fixture stopped rejecting")
        elif old_status != "rejected" and new_status == "rejected":
            improved.append(f"status {old_status} -> {new_status}")
    else:
        old_rank = STATUS_RANK.get(old_status, 0)
        new_rank = STATUS_RANK.get(new_status, 0)
        if new_rank < old_rank:
            regressed.append(f"status {old_status} -> {new_status}")
        elif new_rank > old_rank:
            improved.append(f"status {old_status} -> {new_status}")
    if before["expected_satisfied"] is True and after["expected_satisfied"] is False:
        regressed.append("rejected without its expected fragment")
    elif before["expected_satisfied"] is False and after["expected_satisfied"] is True:
        improved.append("expected fragment now satisfied")
    old_facts, new_facts = before["facts"], after["facts"]
    old_stdout, new_stdout = old_facts.get("stdout"), new_facts.get("stdout")
    if old_stdout == "match" and new_stdout != "match":
        regressed.append("stdout match -> " + (new_stdout or "absent"))
    elif old_stdout == "differs" and new_stdout == "match":
        improved.append("stdout differs -> match")
    if old_facts.get("exit") != new_facts.get("exit"):
        moved.append(f"exit {old_facts.get('exit')} -> {new_facts.get('exit')}")
    elif old_facts != new_facts:
        moved.append("facts moved")
    if (corpus_records.normalized_diagnostics(before["diagnostics"])
            != corpus_records.normalized_diagnostics(after["diagnostics"])):
        moved.append("diagnostics moved")
    if regressed:
        return "regressed", regressed + improved + moved
    if improved:
        return "improved", improved + moved
    if moved:
        return "moved", moved
    # Perf moves only apply to otherwise-clean fixtures: a structural move
    # already explains the fixture and the timing is noise on top.
    old_ms, new_ms = before.get("millis", 0), after.get("millis", 0)
    if old_ms >= 2000 and new_ms >= 2 * old_ms and new_ms - old_ms >= 5000:
        return "perf_slower", [f"{old_ms}ms -> {new_ms}ms (>=2x slowdown)"]
    if old_ms >= 2000 and new_ms <= old_ms // 2 and old_ms - new_ms >= 5000:
        return "perf_faster", [f"{old_ms}ms -> {new_ms}ms (>=2x faster)"]
    return "clean", []


def move_classes(diffs: list) -> str:
    """Separate RECORDING DEBT from behaviour that moved.

    A fixture added, removed or repaired by another lane prints as a move
    for every developer until its golden row catches up, and that debt
    accumulates: it is most of what an unfiltered run shows on a clean
    checkout. Naming the classes lets a reader see in one line whether the
    run is telling them something about their own change. Classes derive
    from classify() verdicts so this line cannot disagree with the exit
    code — the old hand-rolled mapping called a fail fixture that stopped
    rejecting "improved", which pinned a soundness regression as progress.
    """
    labels = {"new": "fixture added, golden not recorded",
              "removed": "fixture removed, golden row left behind",
              "improved": "improved, golden not re-recorded",
              "regressed": "REGRESSED",
              "moved": "diagnostics or facts moved",
              "perf_slower": "diagnostics or facts moved",
              "perf_faster": "diagnostics or facts moved"}
    classes = collections.Counter(labels[diff["verdict"]] for diff in diffs)
    if not classes:
        return "corpus_gate: no structured diffs to classify"
    parts = ", ".join(f"{count} {name}"
                      for name, count in classes.most_common())
    return f"corpus_gate: of those — {parts}"


def shown(value) -> str:
    """One field of a record, as a diff line shows it."""
    if isinstance(value, list):
        return " | ".join(value) if value else "(no diagnostics)"
    if isinstance(value, dict):
        return " ".join(f"{key}:{item}" for key, item in value.items()) or "(no facts)"
    return str(value)


def baseline_path(leg: str = "") -> Path:
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
    suffix = f"-{leg}" if leg else ""
    return ROOT / "build" / "corpus_baselines" / f"{revision}{suffix}.txt"


def git_out(*arguments) -> str | None:
    result = subprocess.run(["git", *arguments], cwd=ROOT,
                            capture_output=True, text=True,
                            env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
    return result.stdout.strip() if result.returncode == 0 else None


def commits_between(revision):
    """git rev-list --count <rev>..HEAD; None when the revision is unknown."""
    count = git_out("rev-list", "--count", f"{revision}..HEAD")
    return int(count) if count and count.isdigit() else None


def load_average():
    """1-minute load average, or None where the host has none (Windows)."""
    try:
        return round(os.getloadavg()[0], 2)
    except (AttributeError, OSError):
        return None


def measurement_context(load_start) -> dict:
    """The conditions a corpus run measured under. `jobs` is the worker
    override the runner read (None = runner default). `load1_start` is the
    external load before the gate started; `load1_end` includes the gate's
    own workers (the runner defaults to up to 12), so it is informational
    only and never judges contention. Written into the golden as a
    `# context:` comment by --record, and compared against the golden's own
    context on every diff."""
    return {"schema": "omega-corpus-context-v1",
            "jobs": os.environ.get("OMEGA_CORPUS_JOBS") or None,
            "cpus": os.cpu_count(),
            "platform": platform.system().lower(),
            "machine": platform.machine().lower(),
            "load1_start": load_start,
            "load1_end": load_average()}


def contended(context) -> bool:
    """True when the machine was already busy before the gate started: the
    pre-run load met or exceeded half the hardware's parallelism, so the
    runner's workers competed with outside work and millis inflated for
    reasons unrelated to the compiler. Only `load1_start` counts — the end
    sample carries the gate's own workers, and a 15-fixture run on an idle
    12-cpu host was measured moving it from 5.19 to 8.43; a full run
    saturates it by itself."""
    if not context:
        return False
    cpus = context.get("cpus") or 0
    load = context.get("load1_start")
    return (cpus > 0 and load is not None
            and load >= cpus * CONTENDED_LOAD_FRACTION)


# The runner defaults to one worker per cpu (capped at 12), so any outside
# load competes with it; half the machine already busy is where the
# resulting millis inflation stops being plausibly small. A judgment
# threshold, not a measured one — it only chooses when to print a warning.
CONTENDED_LOAD_FRACTION = 0.5


def perf_context_warnings(golden_context, current_context) -> list[str]:
    """Why this run's perf moves may be measurement noise. Perf moves are
    the only verdict drawn from millis, and millis answer the machine as
    much as the compiler — the warnings name the specific mismatch so the
    reader knows which direction the noise pushed."""
    warnings = []
    if golden_context is None:
        warnings.append(
            "golden predates context stamping — perf moves cannot be "
            "checked against its recording conditions; re-record on a "
            "quiet machine to make them meaningful")
        return warnings
    golden_jobs = golden_context.get("jobs")
    current_jobs = current_context.get("jobs") if current_context else None
    if (golden_jobs is not None and current_jobs is not None
            and str(golden_jobs) != str(current_jobs)):
        warnings.append(
            f"worker counts differ (golden jobs={golden_jobs}, this run "
            f"jobs={current_jobs}) — record and diff at the same "
            "OMEGA_CORPUS_JOBS before trusting perf moves")
    if contended(golden_context):
        warnings.append(
            f"the golden was recorded on a busy machine (pre-run load "
            f"{golden_context.get('load1_start')} on "
            f"{golden_context.get('cpus')} cpus); fixtures only look faster "
            "now — treat perf_faster moves as noise")
    if contended(current_context):
        warnings.append(
            f"this run started on a busy machine (pre-run load "
            f"{current_context.get('load1_start')} on "
            f"{current_context.get('cpus')} cpus); fixtures only look "
            "slower — treat perf_slower moves as noise")
    return warnings


def measurement_stamp(golden) -> str | None:
    """First 12 hex of HEAD, plus `+wt` when tracked files other than the
    golden differ — the row's provenance for `--record`. None without git."""
    revision = git_out("rev-parse", "HEAD")
    if not revision:
        return None
    status = git_out("status", "--porcelain", "--untracked-files=no") or ""
    # `git status --porcelain` writes `XY path` per line, but git_out strips
    # the whole buffer, so a first line like ` M x` arrives as `M x`; parse
    # the path after the status token instead of a fixed column.
    def porcelain_path(line):
        return line.strip().split(None, 1)[-1].split(" -> ")[-1].strip().strip('"')
    # Record targets exempt from provenance dirt: every corpus golden, since a
    # multi-golden --record dirties the earlier goldens while measuring the next.
    record_targets = {
        path.resolve().relative_to(ROOT).as_posix()
        for path in (ROOT / "tests" / "omega").glob("corpus_*.txt")
    }
    dirty = any(
        porcelain_path(line) not in record_targets
        for line in status.splitlines() if line)
    return revision[:12] + ("+wt" if dirty else "")


def verdict_document(options, leg, target, verdict, exit_code, records,
                     classified, oracle_failures, infrastructure_error,
                     golden_provenance=None, perf_context=None):
    """The `--json` verdict document; written on every outcome so a caller
    (or `git bisect run` wrapper) never has to grep the text summary."""
    golden = Path(options.golden)
    try:
        golden_path = golden.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        golden_path = str(golden)
    by_status = collections.Counter(record["status"] for record in records)
    by_verdict = collections.Counter(entry["verdict"] for entry in classified)
    subset = (bool(options.filter) or bool(options.shard)
              or bool(os.environ.get("OMEGA_CORPUS_FIXTURE_FILTER", "").strip())
              or bool(os.environ.get("OMEGA_CORPUS_SHARD", "").strip()))
    document = {
        "schema": "omega-corpus-verdict-v1",
        "revision": git_out("rev-parse", "HEAD"),
        "dirty": bool(git_out("status", "--porcelain",
                              "--untracked-files=no")),
        "leg": leg or "checked",
        "target": target,
        "golden": golden_path,
        "subset": subset,
        "filter": options.filter,
        "shard": options.shard,
        "verdict": verdict,
        "exit_code": exit_code,
        "counts": {
            "fixtures": len(records),
            "by_status": dict(by_status),
            "wrong_reason": sum(1 for record in records
                                if record["expected_satisfied"] is False),
            "fail_not_rejected": sum(
                1 for record in records
                if record["tier"] == "fail" and record["status"] == "checked"),
            "by_verdict": dict(by_verdict)},
        "oracle_failures": oracle_failures,
        "infrastructure_error": infrastructure_error,
        "golden_provenance": golden_provenance,
        "perf_context": perf_context,
        "fixtures": classified}
    return document


def write_verdict(options, document):
    if not options.json:
        return
    path = Path(options.json)
    if path.parent != Path(""):
        path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")


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
                             "the run tier, *_exit and explicit native-exit fixtures, "
                             "and diff against "
                             "tests/omega/corpus_native_<target>.txt")
    parser.add_argument("--native-target", metavar="TARGET",
                        help="implies --native: build for TARGET instead of the "
                             "host (macos_x86_64 executes under Rosetta on an "
                             "Apple-silicon host) and diff against "
                             "tests/omega/corpus_native_<TARGET>.txt")
    parser.add_argument("--interpret", action="store_true",
                        help="also execute runnable fixtures on the checked "
                             "interpreter and diff against "
                             "tests/omega/corpus_interpreted.txt")
    parser.add_argument("--both", action="store_true",
                        help="run the native and interpreted legs together, "
                             "record whether they agree, and diff against "
                             "tests/omega/corpus_both_<target>.txt")
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
    parser.add_argument("--write-moves", type=Path, metavar="PATH",
                        help="write every moved fixture's BEFORE and AFTER "
                             "records to PATH as JSON, untruncated — the "
                             "summary below cuts each field at 300 columns, "
                             "which hides the diagnostic that actually moved")
    parser.add_argument("--bisect", action="store_true",
                        help="exit for `git bisect run`: 1 on regressed "
                             "(bad), 125 on infrastructure failure (skip), "
                             "0 otherwise (good), 255 on usage error (abort)")
    parser.add_argument("--json", metavar="PATH",
                        help="write the machine-readable verdict document "
                             "to PATH on every outcome, including "
                             "infrastructure failure and --record refusal")
    try:
        options = parser.parse_args()
    except SystemExit as error:
        if not error.code:
            return 0
        # Under `git bisect run` every code in 1..127 (except 125) means
        # "bad", so a usage error would convict an innocent commit.
        return 255 if "--bisect" in sys.argv else error.code
    leg = ""
    target = None
    # `git bisect run` reads 1..127 (except 125) as "bad"; usage errors say
    # nothing about the commit under test, so they abort instead.
    usage_error = 255 if options.bisect else 2
    if options.both:
        options.native = True
        options.interpret = True
    if options.native_target:
        options.native = True
        os.environ["OMEGA_CORPUS_NATIVE_TARGET"] = options.native_target
    if options.interpret:
        os.environ["OMEGA_CORPUS_INTERPRET"] = "1"
        leg = "interpreted"
        if options.golden == GOLDEN and not options.native:
            options.golden = ROOT / "tests" / "omega" / "corpus_interpreted.txt"
    if options.native:
        target = options.native_target or host_target()
        if target is None:
            print("corpus_gate: this host has no catalogued Omega target for --native")
            return usage_error
        os.environ["OMEGA_CORPUS_NATIVE"] = "1"
        leg = "both" if options.interpret else "native"
        if options.golden == GOLDEN:
            name = "both" if options.interpret else "native"
            options.golden = ROOT / "tests" / "omega" / f"corpus_{name}_{target}.txt"
    if options.baseline:
        if options.golden != GOLDEN and not leg:
            print("corpus_gate: --baseline and --golden name different goldens")
            return usage_error
        # A foreign native target keeps its own baseline beside the host's.
        options.golden = baseline_path(
            f"{leg}-{options.native_target}" if options.native_target else leg)
        if not options.golden.is_file() and not options.record:
            print(f"corpus_gate: no baseline at {options.golden.relative_to(ROOT)}; "
                  "record one from a clean tree first:\n"
                  "  git stash && python3 tools/corpus_gate.py --baseline --record "
                  "&& git stash pop")
            return usage_error
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
    # Sampled before and after the run so the recorded context brackets the
    # measurement; millis answer the machine as much as the compiler.
    load_start = load_average()
    try:
        binary = options.runner or build_runner()
        records = run_runner(binary)
        context = measurement_context(load_start)
    except InfrastructureFailure as error:
        print(str(error), file=sys.stderr)
        code = 125 if options.bisect else 4
        write_verdict(options, verdict_document(
            options, leg, target, "infrastructure_failed", code, [], [],
            [], str(error)))
        return code
    failures = []
    if options.native:
        failures.extend(native_exit_failures(records, ROOT / "tests" / "omega", target))
        failures.extend(execution_stdout_failures(
            records, ROOT / "tests" / "omega", "native"))
    if options.interpret:
        failures.extend(execution_stdout_failures(
            records, ROOT / "tests" / "omega", "interpreter"))
    if failures:
        for failure in failures:
            print(f"corpus_gate: {failure}", file=sys.stderr)
        code = 1 if options.bisect else 3
        write_verdict(options, verdict_document(
            options, leg, target, "regressed", code, records, [],
            failures, None))
        return code
    # --record stamps each row measured this run with its provenance; rows
    # merged forward in a subset record keep the stamp they were born with.
    stamp = measurement_stamp(options.golden) if options.record else None
    if stamp:
        for record in records:
            record["recorded_at"] = stamp
    rendered = corpus_records.render(records, context)

    if options.record:
        # A subset record merges into the golden: fixtures that ran get their
        # fresh records, everything else keeps its pin — so sharded/filtered
        # recordings assemble the same file an unfiltered record would. The
        # context line describes this run, not the merged rows' originals —
        # which is honest, since load context only says when the file was
        # written, and each row's @stamp still says when it was measured.
        if subset and options.golden.is_file():
            prior = {record["fixture"]: record
                     for record in corpus_records.read(options.golden)}
            prior.update({record["fixture"]: record for record in records})
            merged = [prior[fixture] for fixture in sorted(prior)]
            rendered = corpus_records.render(merged, context)
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
        write_verdict(options, verdict_document(
            options, leg, target, "recorded", 0, records, [], [], None))
        return 0

    if not options.golden.is_file():
        print(f"corpus_gate: no golden at {options.golden}; "
              f"run --record first", file=sys.stderr)
        return usage_error
    golden_context = corpus_records.read_context(options.golden)
    golden_records = corpus_records.read(options.golden)
    golden = {record["fixture"]: record for record in golden_records}
    actual = {record["fixture"]: record for record in records}

    structural_moves = []
    structured_diffs = []
    classified = []
    perf_moves = []
    perf_notes = []
    for fixture, record in actual.items():
        before = golden.get(fixture)
        verdict, reasons = classify(before, record)
        if verdict != "clean":
            classified.append({"fixture": fixture, "verdict": verdict,
                               "reasons": reasons,
                               "before": before, "after": record})
        if before is None:
            structural_moves.append(f"+ {fixture}: new fixture "
                                  f"({record['status']})")
            structured_diffs.append(
                {"fixture": fixture, "verdict": verdict,
                 "before": {"status": "absent"},
                 "after": {"status": record["status"],
                           "expected_satisfied": record["expected_satisfied"],
                           "diagnostics": record["diagnostics"]}})
            continue
        if verdict == "perf_slower":
            perf_moves.append(f"~ {fixture}: {reasons[0]}")
            continue
        if verdict == "perf_faster":
            perf_notes.append(f"  {fixture}: {reasons[0]}")
            continue
        if verdict == "clean":
            continue
        # Show the first field that moved; diagnostics compare normalized
        # (a relabel is not a move) but print their raw text.
        fields = {
            "status": (before["status"], record["status"]),
            "expected_satisfied": (before["expected_satisfied"],
                                   record["expected_satisfied"]),
            "facts": (before["facts"], record["facts"]),
            "diagnostics": (corpus_records.normalized_diagnostics(
                                before["diagnostics"]),
                            corpus_records.normalized_diagnostics(
                                record["diagnostics"]))}
        for field, (old, new) in fields.items():
            if old != new:
                structural_moves.append(
                    f"~ {fixture}: {field} moved\n"
                    f"    - {shown(before[field])[:300]}\n"
                    f"    + {shown(record[field])[:300]}")
                structured_diffs.append(
                    {"fixture": fixture, "verdict": verdict,
                     "before": {"status": before["status"],
                                "expected_satisfied":
                                    before["expected_satisfied"],
                                "diagnostics": before["diagnostics"]},
                     "after": {"status": record["status"],
                               "expected_satisfied":
                                   record["expected_satisfied"],
                               "diagnostics": record["diagnostics"]}})
                break
    if not subset:
        for fixture in golden:
            if fixture not in actual:
                verdict, reasons = classify(golden[fixture], None)
                classified.append({"fixture": fixture, "verdict": verdict,
                                   "reasons": reasons,
                                   "before": golden[fixture], "after": None})
                structural_moves.append(f"- {fixture}: removed from corpus")
                structured_diffs.append(
                    {"fixture": fixture, "verdict": verdict,
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
    verdict_counts = collections.Counter(entry["verdict"]
                                         for entry in classified)
    if verdict_counts:
        summary += f"; verdicts {dict(verdict_counts)}"
    # Golden rows can outlive the commit that measured them when a subset
    # --record merges old rows forward; provenance is the staleness readout
    # for the fixtures this run actually exercised.
    golden_provenance = corpus_records.provenance(
        [golden[fixture] for fixture in actual if fixture in golden],
        commits_between)
    provenance = ("corpus_gate: "
                  + corpus_records.provenance_line(golden_provenance))
    # Perf moves are the only verdict drawn from millis; when the machines
    # or worker counts behind the two measurements disagree, say so instead
    # of letting a busy box read as a compiler slowdown.
    perf_warnings = (perf_context_warnings(golden_context, context)
                     if perf_moves or perf_notes else [])
    perf_context = {"golden": golden_context, "current": context,
                    "warnings": perf_warnings}

    def write_moves() -> None:
        if not options.write_moves:
            return
        options.write_moves.parent.mkdir(parents=True, exist_ok=True)
        options.write_moves.write_text(
            json.dumps(structured_diffs, indent=2) + "\n", encoding="utf-8")
        print(f"corpus_gate: wrote {len(structured_diffs)} moved fixture(s) "
              f"to {options.write_moves}")

    if not structural_moves and not perf_moves:
        print(f"corpus_gate: clean — {summary}")
        print(provenance)
        for line in perf_notes[:20]:
            print(line)
        for warning in perf_warnings:
            print(f"corpus_gate: perf context — {warning}")
        # A clean run still rewrites the file, so a reader never mistakes a
        # previous run's moves for this one's.
        write_moves()
        write_verdict(options, verdict_document(
            options, leg, target, "clean", 0, records, classified, [], None,
            golden_provenance, perf_context))
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
    for warning in perf_warnings:
        print(f"corpus_gate: perf context — {warning}")
    regressed = verdict_counts.get("regressed", 0) > 0
    verdict = "regressed" if regressed else "moved"
    code = (1 if regressed else 0) if options.bisect else \
        (3 if regressed else 1)
    print(f"corpus_gate: {len(structural_moves)} structural move(s), "
          f"{len(perf_moves)} perf move(s) — {summary}")
    print(move_classes(structured_diffs))
    print(provenance)
    write_moves()
    if options.jev:
        jev_advise(structured_diffs)
    write_verdict(options, verdict_document(
        options, leg, target, verdict, code, records, classified, [], None,
        golden_provenance, perf_context))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
