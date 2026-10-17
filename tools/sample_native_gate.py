#!/usr/bin/env python3
"""Run each sample's DOCUMENTED runtime oracle against a native build.

`tools/sample_gate.py` answers whether a sample CHECKS. That is the build
half of SAMPLE-CORPUS at best: a sample can check and still fail native
realization, and it can realize and still print the wrong thing. This gate
answers the other half.

Sample coverage is repository tooling, not a shipped CLI command. Samples carry
no committed lock, so on a fresh checkout compilation stops at `package
acceptance is missing` before lowering. This gate completes the per-checkout
review, then builds, runs, and compares.

The oracle is the sample's own header, which the corpus already documents:

    // Expected exit: 0.
    // Expected output contains: 150

Both lines are optional; a sample with neither is reported as `no-oracle` and
not scored. A program is still built natively without being launched, so a
native-build failure stays visible. A probe under `probes/` with no oracle is
not reviewed or built at all: nothing scores it and package review dominates
gate time. `--build-unscored` builds those probes too. Nothing here is committed: `omega.lock` and `build/` are
ignored, so the acceptance stays local to this checkout, which is what
"review each checkout's package inputs rather than borrowing another
checkout's accepted lock" requires.

Accepting a review records project acceptance, NOT that an audit occurred.
This script accepts every pending decision so the corpus can be measured; do
not mistake a green row here for an audited dependency.

A project that carries `input.txt` beside its entry runs with that file as
its standard input — the corpus runner's fixture convention. Every other
sample launches with empty stdin. When `input.txt` is present, the authored
`expected_native_exit[.<target>].txt`, `expected_stdout.txt` and
`expected_stderr.txt` files are the oracle for each channel they cover and
compare exactly (line endings normalized); a channel without a file falls
back to the header oracle. The `omega run` wrapper lines (`native exit N`
and friends) are CLI reporting on stderr, never program output, so they are
stripped before the stderr comparison.

Discovery is shared with the check gate. Complete intended-invalid controls use
source checking only and require their authored rejection identity. Positive
check-only probes have no runtime score but their failure still fails this gate.

A sample is scored only on the targets it authors. When `build.omg` binds
`<target>::ProgramEntry` for named targets and the selected target is not
among them, the sample is `not-host-runnable` here: SAMPLE-CORPUS judges
documented output "on matching hosts", so a Linux-only sample is not a macOS
failure.

Each result keeps a short `detail` for the console and the complete refusal
in `diagnostic`; the short form cuts the construct name off long lowering
refusals. Every run is also recorded under `--history-dir` (default
`build/sample-gate-runs/`) with its revision, target and summary, so the pass
rate has a history to compare against.

Without `--cli` the gate builds one opt-level 1 CLI per checkout into
`build/gate-cli/` and reuses it incrementally, but each run executes a private
copy of that binary: a run holds the copy open, never the shared image, so two
gates in one checkout can build, measure and rebuild concurrently. The build
and the copy share one lock, because on Windows even a reader blocks the
replacement a rebuild makes.

`--programs-only` selects application work without probe/control trees. Its
completed record feeds the default work queue but does not establish full
release coverage. Unexpected failures are printed immediately, before the
remaining projects finish; a final run record is written only at completion.
"""

import argparse
import atexit
import contextlib
import datetime
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from corpus_gate import host_target
from sample_projects import ROOT, discover as discover_projects, project_at, rejection_verdict, work_kind

# Freestanding targets have no host oracle: a UEFI image cannot run on the
# development host, so scoring it against `Expected exit` would report a
# platform mismatch as a sample defect. Reported, not scored.
NOT_HOST_RUNNABLE = ("samples/uefi/",)
DECISION = re.compile(rb"^(decision row [0-9a-f]+) pending$", re.MULTILINE)
EXPECT_EXIT = re.compile(r"//\s*Expected exit:\s*(\d+)")
EXPECT_OUTPUT = re.compile(r"//\s*Expected output contains:\s*(.+)")
# `omega run` reports the observed host status on its own stderr after
# forwarding the program's streams; these lines are wrapper reporting, never
# program output.
WRAPPER_LINE = re.compile(
    r"native (exit -?\d+|signal \d+|terminated without an exit code)$")
PROGRAM_ENTRY_BINDING = re.compile(r"\b([a-z][a-z0-9_]*)::ProgramEntry\b")
COMMENT = re.compile(r"//[^\n]*|/\*.*?\*/", re.S)
# Console rows and the scored `detail` stay short; `diagnostic` keeps the rest.
DETAIL_LIMIT = 110
NONFAILURE_STATES = ("pass", "checked", "rejected", "no-oracle", "not-host-runnable")


def discover(root, filters=()):
    return [os.path.relpath(project.directory).replace(os.sep, "/")
            for project in discover_projects(Path(root), ROOT, tuple(filters)) if project.role != "library"]


def nested_groups(samples):
    """Samples grouped under their outermost selected ancestor, in order.

    A nested project (a sample's `controls/` or `evidence/` subproject)
    writes its `build/` inside the enclosing package's source tree. Measured
    concurrently with that package, the write lands between the parent's
    review and its run, and the parent fails with "source identity changed"
    or a missing program-entry acceptance that no compiler defect caused.
    Each group runs in one worker; separate groups still run in parallel.
    """
    groups = {}
    for sample in sorted(samples):
        root = next((key for key in groups if sample.startswith(key + "/")), sample)
        groups.setdefault(root, []).append(sample)
    return list(groups.values())


def unscored_fixture_oracles(sample, target):
    """Whether `input.txt` plus authored expected files score this sample."""
    if not os.path.isfile(os.path.join(sample, "input.txt")):
        return False
    authored = fixture_oracles(sample, target)
    return authored is not None and any(authored.values())


def oracle(sample, entry=None):
    text = Path(entry or os.path.join(sample, "main.omg")).read_text(encoding="utf-8", errors="ignore")
    exit_match = EXPECT_EXIT.search(text)
    output_match = EXPECT_OUTPUT.search(text)
    return (
        int(exit_match.group(1)) if exit_match else None,
        output_match.group(1).strip() if output_match else None,
    )


def bound_targets(sample):
    """Targets whose program-entry slot `build.omg` binds by name.

    Only explicit `<target>::ProgramEntry` spellings count. A build that binds
    through some other construction yields no names, and the sample is then
    measured on every target as before rather than skipped on a guess.
    """
    build = Path(sample, "build.omg")
    if not build.is_file():
        return set()
    text = COMMENT.sub("", build.read_text(encoding="utf-8", errors="ignore"))
    return set(PROGRAM_ENTRY_BINDING.findall(text))


# Every child the gate launches is decoded the same way: UTF-8 — the encoding
# of the compiler's diagnostics and of the oracle files — with replacement for
# bytes that are not valid UTF-8. `text=True` alone decodes through the host
# code page (cp1252 on Windows) under strict errors, and one byte outside it —
# witnessed 0x90 in compiler/mbx output on 2026-10-16 — killed the reader
# thread inside subprocess.run, losing the child's streams and aborting the
# gate mid-run.
CHILD_TEXT = {"text": True, "encoding": "utf-8", "errors": "replace"}

# Each worker thread measures one sample at a time, so the time of every
# compiler call it makes is attributed to that sample's phases.
_phases = threading.local()


def phase_of(command):
    """`omega update` is review, `--check` is checking, `run` builds and
    executes, and a bare `--target` compile publishes a native product."""
    word = command[1] if len(command) > 1 else ""
    return {"update": "review", "--check": "check", "run": "run"}.get(word, "build")


def run(command, timeout, stdin=subprocess.DEVNULL, cwd=None):
    start = time.monotonic()
    try:
        return subprocess.run(
            command,
            capture_output=True,
            timeout=timeout,
            stdin=stdin,
            cwd=cwd,
            **CHILD_TEXT,
        )
    except subprocess.TimeoutExpired:
        return None
    finally:
        times = getattr(_phases, "times", None)
        if times is not None:
            phase = phase_of(command)
            times[phase] = times.get(phase, 0.0) + time.monotonic() - start


# Refusals about this checkout's own package state, not the program: an
# accepted lock or proposal left by an interrupted run, by a CLI built from
# another revision, or by a concurrent write into the package's sources.
STALE_PACKAGE_STATE = re.compile(
    r"needs an accepted program-entry binding"
    r"|source identity changed"
    r"|resume-failed: local source")
# Local package state the gate itself creates; all of it is ignored by Git.
PACKAGE_STATE = ("build", "omega.lock", "omega.admissions")


def clear_package_state(sample):
    for name in PACKAGE_STATE:
        path = os.path.join(sample, name)
        if os.path.isdir(path):
            shutil.rmtree(path, ignore_errors=True)
        elif os.path.exists(path):
            os.remove(path)


def timed_measure(cli, sample, target, timeout, build_unscored=False):
    """`measure` plus the wall time it took and where that time went.

    A failure caused by stale local package state is measured once more from
    a clean review. A compiler defect reproduces on that clean retry, so the
    retry cannot hide one; `retried` keeps the first refusal visible.
    """
    _phases.times = {}
    start = time.monotonic()
    try:
        result = measure(cli, sample, target, timeout, build_unscored)
        first = result.get("diagnostic") or result.get("detail", "")
        if result["state"] in ("no-native-build", "error") and STALE_PACKAGE_STATE.search(first):
            clear_package_state(sample)
            result = measure(cli, sample, target, timeout, build_unscored)
            result["retried"] = first
    finally:
        times, _phases.times = _phases.times, None
    result["seconds"] = round(time.monotonic() - start, 2)
    result["phases"] = {phase: round(seconds, 2) for phase, seconds in sorted(times.items())}
    if result["state"] not in NONFAILURE_STATES:
        # Do not wait for hundreds of unrelated probes before exposing work.
        # Aggregate results and the completed run record remain authoritative.
        print(f"sample_native_gate: {result['state']}: {sample}: "
              f"{result.get('diagnostic') or result.get('detail', '')}",
              file=sys.stderr, flush=True)
    return result


def fixture_oracles(sample, target):
    """Corpus-runner-style oracle files beside a project entry.

    `expected_native_exit.<target>.txt` overrides the portable
    `expected_native_exit.txt`, matching the corpus gate's target-specific
    oracle spelling. Returns the authored file per channel, or None.
    """
    exit_path = None
    for name in (f"expected_native_exit.{target}.txt", "expected_native_exit.txt"):
        candidate = os.path.join(sample, name)
        if os.path.isfile(candidate):
            exit_path = candidate
            break
    return {
        "exit": exit_path,
        "stdout": os.path.join(sample, "expected_stdout.txt")
                  if os.path.isfile(os.path.join(sample, "expected_stdout.txt")) else None,
        "stderr": os.path.join(sample, "expected_stderr.txt")
                  if os.path.isfile(os.path.join(sample, "expected_stderr.txt")) else None,
    }


def normalized(path):
    """Oracle files compare byte-exact except for line-ending normalization."""
    return Path(path).read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n")


def program_stderr(stderr):
    """Strip `omega run` wrapper lines; what remains is the program's stderr."""
    lines = stderr.replace("\r\n", "\n").split("\n")
    return "\n".join(line for line in lines if not WRAPPER_LINE.match(line))


def accept_review(cli, sample, target, timeout):
    """Complete the per-checkout package review.

    Returns a failure reason (or None) and the review's own output, which is
    where a package that fails checking reports why.
    """
    completed = run([cli, "update", "--project", sample, "--target", target], timeout)
    if completed is None:
        return "review-timeout", ""
    output = completed.stdout + completed.stderr
    if "unsupported package lock version" in output:
        # The lock is this checkout's own earlier acceptance, written by a CLI
        # built from another revision; the compiler asks for it to be removed
        # explicitly before it will review afresh. It is ignored, local state,
        # so removing it loses nothing the gate's review would not recreate.
        lock = os.path.join(sample, "omega.lock")
        if os.path.exists(lock):
            os.remove(lock)
        completed = run([cli, "update", "--project", sample, "--target", target], timeout)
        if completed is None:
            return "review-timeout", ""
        output = completed.stdout + completed.stderr
    review = os.path.join(sample, "build", "package-manager", f"review-{target}.txt")
    text = b""
    if os.path.exists(review):
        with open(review, "rb") as document:
            text = document.read()
    if completed.returncode != 0 and "error:" in output and not DECISION.search(text):
        # The candidate failed checking, for example a binding field without
        # a provider on this target, so no decision awaits acceptance. `omega
        # run` would later report only the missing acceptance; keep this one.
        return "review-failed", output
    if DECISION.search(text):
        # Review framing is byte-sensitive and LF-only, including on Windows.
        # Change only decision tokens; never transcode findings or newlines.
        with open(review, "wb") as document:
            document.write(DECISION.sub(rb"\1 accept", text))
        resumed = run([cli, "update", "--project", sample, "--resume"], timeout)
        if resumed is None:
            return "resume-timeout", output
        if resumed.returncode != 0:
            return "resume-failed", resumed.stdout + resumed.stderr
    return None, output


def classify(message, limit=DETAIL_LIMIT):
    """The refusal a failure reports, cut to `limit` characters.

    `limit=None` returns the whole refusal: the `diagnostic` field, where
    a lowering refusal's construct name survives.
    """
    message = " ".join(message.split())

    def cut(text):
        return text if limit is None else text[:limit]

    if "byte replacement requires bounded bytes" in message:
        return "byte replacement (predicate carrier)"
    # The diagnostic arrives with escaped quotes inside a Debug-formatted
    # Vec<Diagnostic>, so the inner reason is `Unsupported(\"...\")`.
    match = re.search(r'Unsupported\(\\?"([^"\\]*)', message)
    if match:
        return cut(f"lowering unsupported: {match.group(1)}")
    match = re.search(r'message: \\?"([^"\\]*)', message)
    if match:
        return cut(f"realization: {match.group(1)}")
    if "omitted at local construction at" in message:
        phase = re.search(r"omitted at local construction at `([^`]*)`", message)
        return f"does not check: {phase.group(1) if phase else '?'}"
    # A lock this checkout minted earlier stops matching once its sources
    # change, and `omega update` cannot refresh it while the package fails
    # checking; `omega run` then reports only the stale lock. Treat it as an
    # incomplete review so the checker's own refusal is reported instead.
    if (
        "package acceptance is missing" in message
        or "package review" in message
        or "cannot prepare accepted omega.lock" in message
    ):
        return "package review incomplete"
    # A checked-compilation failure leads with a wrapper line and carries the
    # real refusal on the following `error:` line; report that, not the wrapper.
    # The message is already flattened to one line, so split on each `error:`
    # rather than matching to the end of the line, which kept the wrapper.
    wrappers = (
        "cannot compile accepted package inputs",
        "checked compilation failed for package",
        "cannot check prepared project",
    )
    errors = [
        part.split(" source-unit ")[0].strip()
        for part in re.split(r"\berror: ", message)[1:]
    ]
    refusals = [error for error in errors if not error.startswith(wrappers)]
    if refusals or errors:
        return cut((refusals or errors)[0])
    return cut(message.strip()) or "unknown"


def failure(sample, state, message, prefix=""):
    """A failing result: short `detail` for the console, whole `diagnostic`."""
    return {
        "sample": sample,
        "state": state,
        "detail": prefix + classify(message),
        "diagnostic": prefix + classify(message, limit=None),
    }


def check_refusal(cli, sample, timeout):
    """The real reason a sample never reached a review: it does not check.

    `omega update` cannot produce a review for a package that fails checking,
    so the review file never appears and `omega run` reports the missing
    acceptance. Reporting that would blame the gate's own review step for a
    sample the check gate already refuses, so ask the checker instead.
    Returns the checker's output, or None on timeout.
    """
    completed = run([cli, "--check", os.path.join(sample, "main.omg")], timeout)
    if completed is None:
        return None
    return completed.stdout + completed.stderr


def measure(cli, sample, target, timeout, build_unscored=False):
    project = project_at(Path(sample), ROOT)
    if project.entry is None:
        return {"sample": sample, "state": "discovery-error", "detail": "missing unambiguous entry; declare sample.json entry"}
    if project.role == "intended-invalid":
        # Negative controls check their source contract and never enter package
        # acceptance or native execution. A refusal for another reason is red.
        completed = run([cli, "--check", "--target", target, str(project.entry)], timeout)
        if completed is None:
            return {"sample": sample, "role": project.role, "state": "timeout", "detail": f">{timeout}s"}
        matched, detail = rejection_verdict(project, completed.returncode, completed.stderr)
        return {"sample": sample, "role": project.role, "state": "rejected" if matched else "rejection-failed", "detail": detail}
    if any(sample.startswith(prefix) for prefix in NOT_HOST_RUNNABLE):
        return {
            "sample": sample,
            "state": "not-host-runnable",
            "detail": "freestanding target; no host oracle",
        }
    targets = bound_targets(sample)
    if targets and target not in targets:
        # The sample authors other targets only; measuring it here would
        # report its missing binding as a compiler failure on this host.
        return {
            "sample": sample,
            "state": "not-host-runnable",
            "detail": f"binds ProgramEntry only for {', '.join(sorted(targets))}",
        }
    expected_exit, expected_output = oracle(sample, project.entry)
    if (not build_unscored and "probes" in Path(sample).parts
            and project.role not in ("check-only", "library")
            and expected_exit is None and expected_output is None
            and not unscored_fixture_oracles(sample, target)):
        # A probe with no oracle pins nothing the gate can score; its package
        # review and native build cost minutes and decided nothing. A program
        # outside probes/ is still built so a native-build failure shows.
        return {"sample": sample, "state": "no-oracle",
                "detail": "probe states no oracle; not built (--build-unscored builds it)"}
    reason, review_output = accept_review(cli, sample, target, timeout)
    if reason == "review-failed":
        # A candidate that fails checking cannot build natively; report why.
        # A check-only probe that fails checking stays a check failure.
        state = "check-fail" if project.role in ("check-only", "library") else "no-native-build"
        return failure(sample, state, review_output, prefix="package review: ")
    if reason:
        if review_output:
            return failure(sample, "error", review_output, prefix=reason + ": ")
        return {"sample": sample, "state": "error", "detail": reason}
    if project.role in ("check-only", "library"):
        # A check-only probe: report whether it checks, never as a native
        # build failure. It is not scored against a runtime oracle.
        completed = run([cli, "--check", str(project.entry)], timeout)
        if completed is None:
            return {"sample": sample, "state": "timeout", "detail": f">{timeout}s"}
        if completed.returncode != 0:
            return failure(sample, "check-fail", completed.stdout + completed.stderr)
        return {"sample": sample, "state": "checked", "detail": "check-only: binds no program entry"}
    # A project carrying `input.txt` runs under the corpus-runner fixture
    # contract: the file is the launched program's stdin. Its authored
    # `expected_*` files then score their channels exactly; a channel without
    # a file still falls back to the header oracle. Without `input.txt` the
    # files describe a private harness run this gate cannot reproduce, so
    # scoring stays on the header alone.
    input_path = os.path.join(sample, "input.txt")
    has_input = os.path.isfile(input_path)
    authored = fixture_oracles(sample, target) if has_input else None
    fixture = authored is not None and any(authored.values())
    if not fixture and expected_exit is None and expected_output is None:
        # Nothing to compare, so publish the native product without launching
        # it: an interactive sample would wait for input until the timeout.
        completed = run([cli, "--target", target, str(project.entry)], timeout)
        if completed is None:
            return {"sample": sample, "state": "timeout", "detail": f">{timeout}s"}
        if completed.returncode != 0:
            return failure(sample, "no-native-build", completed.stdout + completed.stderr)
        return {"sample": sample, "state": "no-oracle", "detail": "header documents neither"}
    input_stream = open(input_path, "rb") if has_input else subprocess.DEVNULL
    try:
        # Programs write relative paths (WAV files, note_vault's directory)
        # into their working directory. A per-sample scratch directory under
        # the ignored build/ keeps that output out of the checkout and away
        # from other samples running at the same time.
        scratch = os.path.join(sample, "build", "gate-run")
        os.makedirs(scratch, exist_ok=True)
        completed = run([cli, "run", str(project.entry)], timeout, stdin=input_stream, cwd=scratch)
    finally:
        if input_stream is not subprocess.DEVNULL:
            input_stream.close()
    if completed is None:
        return {"sample": sample, "state": "timeout", "detail": f">{timeout}s"}
    combined = completed.stdout + completed.stderr
    if "native compile FAILED" in combined or "cannot realize" in combined:
        message, prefix = combined, ""
        if classify(message) == "package review incomplete":
            message = check_refusal(cli, sample, timeout)
            if message is None:
                return {"sample": sample, "state": "no-native-build", "detail": "check-timeout"}
        # `--check` of a package with a stale lock reports only the lock too;
        # the review step's own output then carries the checking refusal.
        if classify(message) == "package review incomplete" and "error:" in review_output:
            message, prefix = review_output, "does not check: "
        return failure(sample, "no-native-build", message, prefix)
    mismatches = []
    if fixture and authored["exit"] is not None:
        expectation = normalized(authored["exit"]).strip()
        if expectation == "trap":
            # The corpus trap oracle: SIGTRAP on ARM64, SIGILL on Unix
            # x86-64, STATUS_ILLEGAL_INSTRUCTION on Windows x86-64 — read
            # through the `omega run` wrapper's host status line.
            if not re.search(
                    r"native (signal [45]|exit -1073741795)", completed.stderr):
                mismatches.append("expected native trap; run exited without an illegal-instruction signal")
        else:
            try:
                expected_code = int(expectation)
            except ValueError:
                mismatches.append(f"invalid native exit expectation {expectation!r}")
            else:
                if completed.returncode != expected_code:
                    mismatches.append(
                        f"exit {completed.returncode} != expected_native_exit.txt {expected_code}")
    elif expected_exit is not None and completed.returncode != expected_exit:
        mismatches.append(f"exit {completed.returncode} != {expected_exit}")
    if fixture and authored["stdout"] is not None:
        if normalized(authored["stdout"]) != completed.stdout.replace("\r\n", "\n"):
            mismatches.append("stdout differs from expected_stdout.txt")
    elif expected_output is not None and expected_output not in completed.stdout:
        mismatches.append(f"stdout lacks {expected_output!r}")
    if fixture and authored["stderr"] is not None:
        if normalized(authored["stderr"]) != program_stderr(completed.stderr):
            mismatches.append("stderr differs from expected_stderr.txt")
    if mismatches:
        return {"sample": sample, "state": "oracle-failed", "detail": "; ".join(mismatches)}
    return {"sample": sample, "state": "pass", "detail": ""}


def shared_runs_directory():
    """Run records live in the main checkout's `build/sample-gate-runs/`, shared
    by every worktree of the repository: agents work in worktrees, and a record
    written inside one was invisible to `tools/sample_queue.py` in all others."""
    completed = subprocess.run(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
        cwd=os.path.dirname(os.path.abspath(__file__)), capture_output=True, **CHILD_TEXT)
    if completed.returncode != 0 or not completed.stdout.strip():
        return os.path.join("build", "sample-gate-runs")
    return os.path.join(os.path.dirname(completed.stdout.strip()), "build", "sample-gate-runs")


def find_work(arguments, samples):
    """Measure available applications until one real refusal supplies work.

    This is discovery coverage, never a release result. Serial scheduling means
    finding an outcome cannot leave unrelated compiler children to drain.
    Previous failures guide ordering; every returned outcome is measured again.
    """
    import sample_queue

    try:
        ownership = sample_queue.live_ownership()
    except sample_queue.outcome_claims.ClaimsError as error:
        ownership = None
        print(f"sample_native_gate: ownership unknown: {error}", file=sys.stderr)
    previous = sample_queue.latest_record()
    record = json.loads(Path(previous).read_text(encoding="utf-8")) if previous else {}
    candidates = sample_queue.discovery_candidates(samples, record, ownership)
    results = []
    for sample in candidates:
        result = timed_measure(arguments.cli, sample, arguments.target,
                               arguments.timeout, arguments.build_unscored)
        results.append(result)
        if sample_queue.queue({"results": [result]}, live_claims=ownership):
            break
    return results


def escape_unencodable_output():
    """Let refusals quoting child output print on any redirected stream.

    A refusal carries the child's own text; on a stream encoded to a code page
    that lacks one of its characters — a cp1252 pipe on Windows — printing it
    raises UnicodeEncodeError, the same gate death moved to the output side.
    Backslash escapes keep the text readable on every stream. stderr already
    defaults to them; reconfiguring both makes the policy explicit.
    """
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(errors="backslashreplace")


def main():
    escape_unencodable_output()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli",
                        help="compiler to measure; by default the gate builds an "
                             "opt-level 1 CLI into build/gate-cli/ and executes a "
                             "private copy of it")
    parser.add_argument("--target", default=host_target(),
                        help="defaults to this host's target")
    parser.add_argument("--filter", help="comma-separated path fragments")
    parser.add_argument("--programs-only", action="store_true",
                        help="discover application work without probe/control trees; "
                             "not a full release gate")
    parser.add_argument("--find-work", action="store_true",
                        help="remeasure likely unclaimed application failures first, then "
                             "small applications; stop at the first confirmed outcome "
                             "(partial discovery coverage, not a release gate)")
    parser.add_argument("--build-unscored", action="store_true",
                        help="review and natively build samples that state no oracle "
                             "(default: report them as no-oracle without compiling)")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=300, help="seconds per CLI call")
    parser.add_argument("--write-results", metavar="PATH")
    parser.add_argument("--history-dir", default=shared_runs_directory(),
                        help="directory receiving one dated record per run (default: the "
                             "main checkout's build/sample-gate-runs, shared by every "
                             "worktree); empty disables")
    arguments = parser.parse_args()
    if arguments.find_work and (not arguments.programs_only or arguments.filter):
        parser.error("--find-work requires --programs-only and no --filter")
    if arguments.find_work:
        arguments.jobs = 1
    started = datetime.datetime.now(datetime.timezone.utc)
    if arguments.cli is None:
        arguments.cli = build_gate_cli()
        if arguments.cli is None:
            return 2
        # The run record cites the durable shared build; the processes execute
        # a private copy so a concurrent gate's incremental build can replace
        # the shared image without racing this run.
        arguments.measured_cli = arguments.cli
        arguments.cli = snapshot_cli(arguments.cli)
    else:
        # Programs run from per-sample scratch directories, so a relative
        # compiler path would no longer resolve.
        arguments.cli = os.path.abspath(arguments.cli)
        arguments.measured_cli = arguments.cli
    staleness = cli_staleness(arguments.measured_cli)
    if staleness:
        print(f"sample_native_gate: warning: {staleness}", file=sys.stderr)

    wanted = [fragment.strip() for fragment in (arguments.filter or "").split(",") if fragment.strip()]
    samples = discover("samples", wanted)
    if arguments.programs_only:
        samples = [sample for sample in samples if work_kind(sample) == "program"]
    if not samples:
        print("sample_native_gate: nothing selected", file=sys.stderr)
        return 2

    wall_start = time.monotonic()
    if arguments.find_work:
        results = find_work(arguments, samples)
        print(f"sample_native_gate: discovery measured {len(results)}/{len(samples)} "
              "applications; partial coverage cannot establish release health")
    else:
        with ThreadPoolExecutor(max_workers=arguments.jobs) as pool:
            grouped = pool.map(
                lambda group: [timed_measure(arguments.cli, s, arguments.target, arguments.timeout,
                                             arguments.build_unscored)
                               for s in group],
                nested_groups(samples),
            )
            results = sorted((result for group in grouped for result in group),
                             key=lambda result: result["sample"])
    wall_seconds = time.monotonic() - wall_start

    buckets = {}
    for result in results:
        buckets.setdefault(result["state"], []).append(result)
    for state in sorted(buckets, key=lambda s: -len(buckets[s])):
        rows = buckets[state]
        print(f"\n== {state}: {len(rows)}")
        for row in sorted(rows, key=lambda r: r["sample"]):
            print(f"   {row['sample']:52} {row['detail'][:104]}")

    check_only = ("checked", "check-fail")
    negatives = [result for result in results if result.get("role") == "intended-invalid"]
    scored = [
        r
        for r in results
        if r.get("role") != "intended-invalid"
        and r["state"] not in ("no-oracle", "not-host-runnable", *check_only)
    ]
    passed = len(buckets.get("pass", []))
    probes = sum(len(buckets.get(state, [])) for state in check_only)
    print(
        f"\nsample_native_gate: {passed}/{len(scored)} samples meet their documented "
        f"oracle on {arguments.target} "
        f"({len(results) - len(scored) - probes - len(negatives)} unscored: no oracle or not host-runnable)"
    )
    if probes:
        print(
            f"sample_native_gate: {probes} check-only probes bind no program entry: "
            f"{len(buckets.get('checked', []))} check, "
            f"{len(buckets.get('check-fail', []))} refused"
        )
    if negatives:
        print(f"sample_native_gate: {len(buckets.get('rejected', []))}/{len(negatives)} intended-invalid controls match their authored rejection")
    timing = timing_summary(results, wall_seconds)
    print(
        f"sample_native_gate: {timing['wall_seconds']:.0f}s wall with {arguments.jobs} jobs, "
        f"{timing['sample_seconds']:.0f}s across samples ("
        + ", ".join(f"{phase} {seconds:.0f}s" for phase, seconds in timing["phase_seconds"].items())
        + "); slowest:"
    )
    for row in timing["slowest"][:5]:
        print(f"   {row['seconds']:7.1f}s  {row['sample']}  ({row['state']})")
    retried = [result for result in results if "retried" in result]
    if retried:
        recovered = sum(1 for result in retried if result["state"] in ("pass", "no-oracle"))
        print(f"sample_native_gate: {len(retried)} samples retried after stale package state; "
              f"{recovered} recovered")
    if arguments.write_results:
        output = Path(arguments.write_results)
        output.parent.mkdir(parents=True, exist_ok=True)
        with output.open("w", encoding="utf-8") as document:
            json.dump(results, document, indent=1)
    if arguments.history_dir:
        record_run(arguments, started, results, buckets, passed, len(scored), timing, staleness)
    failures = [result for result in results if result["state"] not in NONFAILURE_STATES]
    return 1 if failures else 0


def timing_summary(results, wall_seconds):
    """Where a run's time went: total, per phase, and the slowest samples."""
    phases = {}
    for result in results:
        for phase, seconds in result.get("phases", {}).items():
            phases[phase] = phases.get(phase, 0.0) + seconds
    slowest = sorted(results, key=lambda result: -result.get("seconds", 0.0))[:20]
    return {
        "wall_seconds": round(wall_seconds, 1),
        "sample_seconds": round(sum(result.get("seconds", 0.0) for result in results), 1),
        "phase_seconds": {phase: round(seconds, 1) for phase, seconds in sorted(phases.items())},
        "slowest": [{"sample": result["sample"], "seconds": result.get("seconds", 0.0),
                     "state": result["state"]} for result in slowest],
    }


# Where the gate's shared, incremental opt-level 1 build lives. The build is
# expensive — a cold compiler is minutes — so every gate in one checkout reuses
# it. Only the built binary is copied per run; see `snapshot_cli`.
GATE_CLI_DIR = ROOT / "build" / "gate-cli"
# One lock for the shared build directory. Cargo serializes concurrent builds
# of the same target directory itself, but the copy out of it (`snapshot_cli`)
# must not overlap another build replacing `debug/omega`; on Windows even a
# plain read handle blocks a replacement (witnessed with `os.replace`), so a
# copy taken while a build replaces the image corrupts the copy or fails the
# build. The lock makes build-and-copy one atomic step across gates while
# leaving execution of the copied binary parallel.
def gate_cli_lock_path():
    """The lock guarding `GATE_CLI_DIR`, beside it in `build/`."""
    return GATE_CLI_DIR.parent / "gate-cli.lock"


# A holder holds the lock for a full incremental build plus a copy. A cold
# build measured ~4.5 minutes under contention, so the bound is a backstop
# against a wedged holder, not a queue policy: the OS lock dies with its
# holder, so a live holder is genuinely building and waiting for it is correct.
GATE_CLI_LOCK_SECONDS = 3600
# The compiler's own sources: a CLI older than the last commit touching
# them was built from a different compiler than this checkout's.
COMPILER_INPUTS = ("omega-rust", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml")


@contextlib.contextmanager
def gate_cli_lock(path=None, seconds=GATE_CLI_LOCK_SECONDS):
    """Blocking, process-scoped lock on Linux/macOS/Windows; never steal.

    A holder that exits releases the OS lock, so a crashed gate cannot wedge
    the next one. The deadline is a diagnostic backstop, not a queue policy:
    a live holder is building and waiting for it is correct.
    """
    path = Path(path) if path is not None else gate_cli_lock_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    stream = open(path, "a+b")
    try:
        stream.seek(0, os.SEEK_END)
        if stream.tell() == 0:
            stream.write(b"\0")
            stream.flush()
        deadline = time.monotonic() + seconds
        while True:
            stream.seek(0)
            try:
                if os.name == "nt":
                    import msvcrt
                    msvcrt.locking(stream.fileno(), msvcrt.LK_NBLCK, 1)
                else:
                    import fcntl
                    fcntl.flock(stream.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except OSError:
                if time.monotonic() >= deadline:
                    raise RuntimeError(
                        f"gate CLI build lock {path} held for more than {seconds}s") from None
                time.sleep(0.5)
        yield
    finally:
        try:
            stream.seek(0)
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(stream.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                import fcntl
                fcntl.flock(stream.fileno(), fcntl.LOCK_UN)
        finally:
            stream.close()


def build_gate_cli():
    """Build the CLI the gate measures by default and return its path.

    testing.md asks for an optimized compiler: package review of a std-using
    sample takes minutes with the unoptimized dev build. Measured on 31
    projects, the opt-level 1 CLI took 66 s where the dev build took 436 s
    with identical results. A dedicated target directory keeps this build
    from replacing the developer's own target/debug/omega. The build is
    incremental, so an unchanged compiler costs seconds.

    The returned path is the shared build output. A run executes a private
    copy of it (`snapshot_cli`), never this path itself, so a concurrent gate
    can reuse and rebuild the shared directory without racing a running
    process. The caller owns the build semantics; the snapshot owns isolation.
    """
    tool = "mbx" if shutil.which("mbx") else "cargo"
    environment = dict(os.environ, CARGO_PROFILE_DEV_OPT_LEVEL="1",
                       CARGO_TARGET_DIR=str(GATE_CLI_DIR))
    print(f"sample_native_gate: building the opt-level 1 CLI into {GATE_CLI_DIR} with {tool}",
          file=sys.stderr)
    # Hold the lock across the build so a rebuild's replacement of
    # `debug/omega` never overlaps another gate's copy out of it.
    with gate_cli_lock():
        completed = subprocess.run([tool, "build", "-p", "omega", "--bin", "omega"],
                                   cwd=ROOT, env=environment, capture_output=True, **CHILD_TEXT)
    if completed.returncode != 0:
        print(completed.stderr[-4000:], file=sys.stderr)
        print("sample_native_gate: building the CLI failed; pass --cli to measure another build",
              file=sys.stderr)
        return None
    name = "omega.exe" if os.name == "nt" else "omega"
    return str(GATE_CLI_DIR / "debug" / name)


def snapshot_cli(cli):
    """Return a private copy of `cli` for this process to execute.

    Two gates in one checkout share the incremental `GATE_CLI_DIR`, and the
    build there rewrites `debug/omega.exe` on every run. A run that executes
    that path holds the image open, so a concurrent gate's incremental build
    cannot replace it and dies the same way Windows refuses any busy image:
    witnessed 2026-10-17 in the main checkout, the second of two gates failed
    with "failed to remove file build/gate-cli/debug/omega.exe: Access is
    denied (os error 5)". The shared directory still caches the expensive
    build; each run copies the built image beside it and executes the copy, so
    a rebuild never races a running process. The copy is removed on exit.

    Copies under the build lock, so the source is not replaced mid-copy.
    """
    with gate_cli_lock():
        os.makedirs(GATE_CLI_DIR.parent, exist_ok=True)
        directory = tempfile.mkdtemp(prefix="gate-cli-run-", dir=str(GATE_CLI_DIR.parent))
        atexit.register(shutil.rmtree, directory, ignore_errors=True)
        target = Path(directory) / Path(cli).name
        shutil.copy2(cli, target)
    return str(target)


def cli_staleness(cli):
    """Why `cli` predates this checkout's compiler, or "" when it does not.

    A stale binary does not exercise the route the checkout describes
    (SAMPLE-CORPUS), so its numbers are not this revision's. Committed
    changes compare commit time with the binary's modification time;
    uncommitted compiler edits compare each edited file's time.
    """
    try:
        built = os.stat(cli).st_mtime
    except OSError:
        return f"CLI {cli} does not exist"

    def git(*command):
        completed = subprocess.run(["git", *command], cwd=ROOT, capture_output=True, **CHILD_TEXT)
        return completed.stdout.strip() if completed.returncode == 0 else ""

    last = git("log", "-1", "--format=%h %ct", "--", *COMPILER_INPUTS).split()
    if len(last) == 2 and int(last[1]) > built:
        return f"CLI {cli} is older than compiler commit {last[0]}; results may not describe this checkout"
    for line in git("status", "--porcelain", "--", *COMPILER_INPUTS).splitlines():
        path = ROOT / line[3:].strip()
        if path.is_file() and path.stat().st_mtime > built:
            return f"CLI {cli} is older than the uncommitted edit to {line[3:].strip()}"
    return ""


def cli_identity(cli):
    """Which compiler binary measured the run. Timings from a debug and an
    optimized build differ by minutes per sample, so a record without this
    cannot be compared with another."""
    path = os.path.realpath(cli)
    try:
        status = os.stat(path)
    except OSError:
        return {"path": path}
    return {
        "path": path,
        "bytes": status.st_size,
        "modified": datetime.datetime.fromtimestamp(
            status.st_mtime, datetime.timezone.utc).isoformat(timespec="seconds"),
    }


def record_run(arguments, started, results, buckets, passed, scored, timing=None, staleness=""):
    """One dated record per run, so the pass rate and its cost have a history.

    The revision and dirty flag say which checkout produced the numbers; a
    filtered run says so, since its counts cover only the selected samples.
    Jobs and the CLI binary say whether two runs' timings are comparable.
    """
    def git(*command):
        completed = subprocess.run(["git", *command], capture_output=True, **CHILD_TEXT)
        return completed.stdout.strip() if completed.returncode == 0 else ""

    record = {
        "started": started.isoformat(timespec="seconds"),
        "finished": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "revision": git("rev-parse", "HEAD"),
        "dirty": bool(git("status", "--porcelain", "--untracked-files=no")),
        "target": arguments.target,
        "cli": dict(cli_identity(getattr(arguments, "measured_cli", arguments.cli)),
                    stale=staleness),
        "jobs": getattr(arguments, "jobs", None),
        "timeout": getattr(arguments, "timeout", None),
        "filter": arguments.filter or "",
        "scope": ("discovery" if getattr(arguments, "find_work", False) else
                  "programs" if getattr(arguments, "programs_only", False) else "all"),
        "summary": {
            "passed": passed,
            "scored": scored,
            "states": {state: len(rows) for state, rows in sorted(buckets.items())},
            "retried": sum(1 for result in results if "retried" in result),
        },
        "timing": timing or {},
        "results": results,
    }
    os.makedirs(arguments.history_dir, exist_ok=True)
    name = f"{started:%Y%m%dT%H%M%SZ}-{arguments.target}.json"
    with open(os.path.join(arguments.history_dir, name), "w") as document:
        json.dump(record, document, indent=1)


if __name__ == "__main__":
    sys.exit(main())
