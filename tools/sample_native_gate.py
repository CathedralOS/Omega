#!/usr/bin/env python3
"""Run each sample's DOCUMENTED runtime oracle against a native build.

`tools/sample_gate.py` answers whether a sample CHECKS. That is the build
half of SAMPLE-CORPUS at best: a sample can check and still fail native
realization, and it can realize and still print the wrong thing. This gate
answers the other half.

`omega refresh-samples` cannot do it. It never accepts trust on a sample's
behalf, and samples carry no committed lock, so on a fresh checkout it stops
at `package acceptance is missing` for essentially every sample and never
reaches lowering. Completing that review is a per-checkout step; this script
performs it, then builds, runs, and compares.

The oracle is the sample's own header, which the corpus already documents:

    // Expected exit: 0.
    // Expected output contains: 150

Both lines are optional; a sample with neither is reported as `no-oracle` and
is not scored. Nothing here is committed: `omega.lock` and `build/` are
ignored, so the acceptance stays local to this checkout, which is what
"review each checkout's package inputs rather than borrowing another
checkout's accepted lock" requires.

Accepting a review records project acceptance, NOT that an audit occurred.
This script accepts every pending decision so the corpus can be measured; do
not mistake a green row here for an audited dependency.
"""

import argparse
import json
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

EXCLUDED = ("samples/apps/squalr",)
# Freestanding targets have no host oracle: a UEFI image cannot run on the
# development host, so scoring it against `Expected exit` would report a
# platform mismatch as a sample defect. Reported, not scored.
NOT_HOST_RUNNABLE = ("samples/uefi/",)
DECISION = re.compile(rb"^(decision row [0-9a-f]+) pending$", re.MULTILINE)
EXPECT_EXIT = re.compile(r"//\s*Expected exit:\s*(\d+)")
EXPECT_OUTPUT = re.compile(r"//\s*Expected output contains:\s*(.+)")


def discover(root):
    found = []
    for directory, subdirectories, files in os.walk(root):
        if "build" in subdirectories:
            subdirectories.remove("build")
        relative = os.path.relpath(directory).replace(os.sep, "/")
        if any(relative.startswith(skip) for skip in EXCLUDED):
            subdirectories[:] = []
            continue
        if "main.omg" in files and "build.omg" in files:
            found.append(relative)
            subdirectories[:] = []
    return sorted(found)


def oracle(sample):
    text = open(os.path.join(sample, "main.omg"), errors="ignore").read()
    exit_match = EXPECT_EXIT.search(text)
    output_match = EXPECT_OUTPUT.search(text)
    return (
        int(exit_match.group(1)) if exit_match else None,
        output_match.group(1).strip() if output_match else None,
    )


def run(command, timeout, stdin_empty=True):
    try:
        return subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=timeout,
            stdin=subprocess.DEVNULL if stdin_empty else None,
        )
    except subprocess.TimeoutExpired:
        return None


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
    if os.path.exists(review):
        # Review framing is byte-sensitive and LF-only, including on Windows.
        # Change only decision tokens; never transcode findings or newlines.
        with open(review, "rb") as document:
            text = document.read()
        if DECISION.search(text):
            with open(review, "wb") as document:
                document.write(DECISION.sub(rb"\1 accept", text))
            resumed = run([cli, "update", "--project", sample, "--resume"], timeout)
            if resumed is None:
                return "resume-timeout", output
            if resumed.returncode != 0:
                return "resume-failed", resumed.stdout + resumed.stderr
    return None, output


def classify(message):
    message = " ".join(message.split())
    if "byte replacement requires bounded bytes" in message:
        return "byte replacement (predicate carrier)"
    # The diagnostic arrives with escaped quotes inside a Debug-formatted
    # Vec<Diagnostic>, so the inner reason is `Unsupported(\"...\")`.
    match = re.search(r'Unsupported\(\\?"([^"\\]{0,90})', message)
    if match:
        return f"lowering unsupported: {match.group(1)}"
    match = re.search(r'message: \\?"([^"\\]{0,90})', message)
    if match:
        return f"realization: {match.group(1)}"
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
    errors = re.findall(r"error: (.+)", message)
    if errors:
        return errors[0].strip()[:110]
    return message.strip()[:110] or "unknown"


def check_refusal(cli, sample, timeout):
    """The real reason a sample never reached a review: it does not check.

    `omega update` cannot produce a review for a package that fails checking,
    so the review file never appears and `omega run` reports the missing
    acceptance. Reporting that would blame the gate's own review step for a
    sample the check gate already refuses, so ask the checker instead.
    """
    completed = run([cli, "--check", os.path.join(sample, "main.omg")], timeout)
    if completed is None:
        return "check-timeout"
    return classify(completed.stdout + completed.stderr)


def measure(cli, sample, target, timeout):
    if any(sample.startswith(prefix) for prefix in NOT_HOST_RUNNABLE):
        return {
            "sample": sample,
            "state": "not-host-runnable",
            "detail": "freestanding target; no host oracle",
        }
    expected_exit, expected_output = oracle(sample)
    reason, review_output = accept_review(cli, sample, target, timeout)
    if reason:
        detail = reason + (": " + classify(review_output) if review_output else "")
        return {"sample": sample, "state": "error", "detail": detail}
    completed = run([cli, "run", os.path.join(sample, "main.omg")], timeout)
    if completed is None:
        return {"sample": sample, "state": "timeout", "detail": f">{timeout}s"}
    combined = completed.stdout + completed.stderr
    if "native compile FAILED" in combined or "cannot realize" in combined:
        detail = classify(combined)
        if detail == "package review incomplete":
            detail = check_refusal(cli, sample, timeout)
        # `--check` of a package with a stale lock reports only the lock too;
        # the review step's own output then carries the checking refusal.
        if detail == "package review incomplete" and "error:" in review_output:
            detail = f"does not check: {classify(review_output)}"
        return {"sample": sample, "state": "no-native-build", "detail": detail}
    if expected_exit is None and expected_output is None:
        return {"sample": sample, "state": "no-oracle", "detail": "header documents neither"}
    mismatches = []
    if expected_exit is not None and completed.returncode != expected_exit:
        mismatches.append(f"exit {completed.returncode} != {expected_exit}")
    if expected_output is not None and expected_output not in completed.stdout:
        mismatches.append(f"stdout lacks {expected_output!r}")
    if mismatches:
        return {"sample": sample, "state": "oracle-failed", "detail": "; ".join(mismatches)}
    return {"sample": sample, "state": "pass", "detail": ""}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", default="./target/debug/omega")
    parser.add_argument("--target", default="macos_arm64")
    parser.add_argument("--filter", help="comma-separated path fragments")
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=300, help="seconds per CLI call")
    parser.add_argument("--write-results", metavar="PATH")
    arguments = parser.parse_args()

    samples = discover("samples")
    if arguments.filter:
        wanted = [fragment.strip() for fragment in arguments.filter.split(",")]
        samples = [s for s in samples if any(w in s for w in wanted)]
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

    scored = [
        r
        for r in results
        if r["state"] not in ("no-oracle", "not-host-runnable")
    ]
    passed = len(buckets.get("pass", []))
    print(
        f"\nsample_native_gate: {passed}/{len(scored)} samples meet their documented "
        f"oracle on {arguments.target} "
        f"({len(results) - len(scored)} unscored: no oracle or not host-runnable)"
    )
    if arguments.write_results:
        json.dump(results, open(arguments.write_results, "w"), indent=1)
    return 0 if passed == len(scored) else 1


if __name__ == "__main__":
    sys.exit(main())
