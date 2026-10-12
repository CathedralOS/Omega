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

Both lines are optional; a sample with neither is built without being
launched, since it may wait for input, and is reported as `no-oracle` and not
scored. Nothing here is committed: `omega.lock` and `build/` are
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
"""

import argparse
import datetime
import json
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from corpus_gate import host_target
from sample_projects import ROOT, discover as discover_projects, project_at, rejection_verdict

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


def discover(root, filters=()):
    return [os.path.relpath(project.directory).replace(os.sep, "/")
            for project in discover_projects(Path(root), ROOT, tuple(filters)) if project.role != "library"]


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


def run(command, timeout, stdin=subprocess.DEVNULL):
    try:
        return subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=timeout,
            stdin=stdin,
        )
    except subprocess.TimeoutExpired:
        return None


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


def measure(cli, sample, target, timeout):
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
        completed = run([cli, "run", str(project.entry)], timeout, stdin=input_stream)
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", default="./target/debug/omega")
    parser.add_argument("--target", default=host_target(),
                        help="defaults to this host's target")
    parser.add_argument("--filter", help="comma-separated path fragments")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=300, help="seconds per CLI call")
    parser.add_argument("--write-results", metavar="PATH")
    parser.add_argument("--history-dir", default=os.path.join("build", "sample-gate-runs"),
                        help="directory receiving one dated record per run; empty disables")
    arguments = parser.parse_args()
    started = datetime.datetime.now(datetime.timezone.utc)

    wanted = [fragment.strip() for fragment in (arguments.filter or "").split(",") if fragment.strip()]
    samples = discover("samples", wanted)
    if not samples:
        print("sample_native_gate: nothing selected", file=sys.stderr)
        return 2

    with ThreadPoolExecutor(max_workers=arguments.jobs) as pool:
        results = list(
            pool.map(
                lambda s: measure(arguments.cli, s, arguments.target, arguments.timeout),
                samples,
            )
        )

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
    if arguments.write_results:
        json.dump(results, open(arguments.write_results, "w"), indent=1)
    if arguments.history_dir:
        record_run(arguments, started, results, buckets, passed, len(scored))
    failures = [result for result in results if result["state"] not in
                ("pass", "checked", "rejected", "no-oracle", "not-host-runnable")]
    return 1 if failures else 0


def record_run(arguments, started, results, buckets, passed, scored):
    """One dated record per run, so the pass rate has a history.

    The revision and dirty flag say which checkout produced the numbers; a
    filtered run says so, since its counts cover only the selected samples.
    """
    def git(*command):
        completed = subprocess.run(["git", *command], capture_output=True, text=True)
        return completed.stdout.strip() if completed.returncode == 0 else ""

    record = {
        "started": started.isoformat(timespec="seconds"),
        "finished": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "revision": git("rev-parse", "HEAD"),
        "dirty": bool(git("status", "--porcelain", "--untracked-files=no")),
        "target": arguments.target,
        "cli": arguments.cli,
        "filter": arguments.filter or "",
        "summary": {
            "passed": passed,
            "scored": scored,
            "states": {state: len(rows) for state, rows in sorted(buckets.items())},
        },
        "results": results,
    }
    os.makedirs(arguments.history_dir, exist_ok=True)
    name = f"{started:%Y%m%dT%H%M%SZ}-{arguments.target}.json"
    with open(os.path.join(arguments.history_dir, name), "w") as document:
        json.dump(record, document, indent=1)


if __name__ == "__main__":
    sys.exit(main())
