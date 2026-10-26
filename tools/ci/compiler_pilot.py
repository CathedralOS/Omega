#!/usr/bin/env python3
"""Build one candidate and require real compiler smoke outcomes for the queue pilot.

This deliberately small gate proves CI plumbing, not release qualification.
The corpus owns source checking, native/Terminal execution and diagnostic oracles;
this caller additionally refuses missing rows and success without execution.
"""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import corpus_gate

POSITIVES = (
    "pass/control_flow/borrowed_state_argument",
    "pass/control_flow/runtime_owned_state_parallel_swap_exit",
    "pass/write_only/state_forwarded_scalar_store_exit",
)
NEGATIVES = (
    "fail/domains/carried_nested_payload_qualification",
    "fail/domains/carried_nested_payload_qualification_missing_arrival",
    "fail/domains/carried_indexed_payload_qualification",
    "fail/domains/carried_runtime_index_payload_qualification",
)


def validate_report(report, expected, *, both, revision, target):
    if (report.get("schema") != "omega-corpus-verdict-v1"
            or report.get("revision") != revision or report.get("dirty") is not False
            or report.get("exit_code") not in (0, 1)
            or report.get("oracle_failures") or report.get("infrastructure_error")):
        raise ValueError("corpus refused, failed an oracle, or measured another source")
    rows = report.get("observations", [])
    if len(rows) != len(expected) or {row["fixture"] for row in rows} != set(expected):
        raise ValueError("corpus observation roster differs from the required fixtures")
    if both and (report.get("leg") != "both" or report.get("target") != target):
        raise ValueError("runtime evidence must use both engines on this host")
    for row in rows:
        if row["fixture"].startswith("fail/"):
            if row["status"] != "rejected" or row.get("expected_satisfied") is not True:
                raise ValueError(f"{row['fixture']}: missing intended rejection")
        elif both:
            facts = row.get("facts", {})
            if (row["status"] != "built" or facts.get("interpreter") != "agrees"
                    or "exit" not in facts or facts["exit"] != facts.get("interpreter-exit")):
                raise ValueError(f"{row['fixture']}: missing native/Terminal agreement")
            for stream in ("stdout", "stderr"):
                oracle = ROOT / "tests" / "omega" / row["fixture"] / f"expected_{stream}.txt"
                if oracle.is_file() and (facts.get(stream) != "match"
                                         or facts.get("interpreter-" + stream) != "match"):
                    raise ValueError(f"{row['fixture']}: missing authored {stream} observation")
        elif row["status"] not in ("checked", "built"):
            raise ValueError(f"{row['fixture']}: positive source did not check")


def command(arguments):
    return subprocess.check_output(arguments, cwd=ROOT, text=True).strip()


def observation_golden(output, *, both, target):
    name = f"corpus_both_{target}.txt" if both else "corpus_outcomes.txt"
    published = ROOT / "tests/omega" / name
    if published.is_file():
        return published
    if not both or not target:
        raise ValueError("source checking requires its published corpus baseline")
    # A first matching-host observation has no historical rows to compare.
    # Keep that absence explicit in the report; fixture oracles and the exact
    # runtime/rejection roster remain mandatory. Never borrow another host's
    # observations or write a fabricated result into the repository's golden.
    initial = output / ("initial-" + name)
    initial.write_text(
        f"# No prior combined observations for {target}; authored oracles apply.\n",
        encoding="utf-8")
    return initial


def execute(output, summary, *, positives=POSITIVES, negatives=NEGATIVES, checking_only=()):
    revision = command(["git", "rev-parse", "HEAD"])
    if command(["git", "status", "--porcelain"]):
        raise ValueError("pilot requires a clean candidate")
    if os.environ.get("GITHUB_SHA", revision) != revision:
        raise ValueError("checkout is not the GitHub candidate being checked")
    summary.update(candidate=revision, target=corpus_gate.host_target(),
                   toolchain=command(["rustc", "-Vv"]), stages=[],
                   fixtures=dict(positive=positives, negative=negatives,
                                 checking_only=checking_only))

    def stage(name, operation):
        started = time.monotonic()
        result = dict(name=name, passed=False)
        summary["stages"].append(result)
        print(f"compiler-pilot: {name}", flush=True)
        try:
            value = operation()
            result["passed"] = True
            return value
        finally:
            result["seconds"] = round(time.monotonic() - started, 3)

    tool = "mbx" if shutil.which("mbx") else "cargo"
    stage("build CLI", lambda: subprocess.run(
        [tool, "build", "--locked", "-p", "omega"], cwd=ROOT,
        stdin=subprocess.DEVNULL, check=True))
    runner = stage("build corpus runner", corpus_gate.build_runner)
    # Keep the exact producer with the reports; later jobs need not rebuild it.
    retained = output / ("corpus_runner.exe" if runner.suffix == ".exe" else "corpus_runner")
    shutil.copy2(runner, retained)
    shutil.copy2(str(runner) + ".compiler-profile.json",
                 str(retained) + ".compiler-profile.json")
    with retained.open("rb") as stream:
        digest = hashlib.sha256()
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    summary["runner_sha256"] = digest.hexdigest()
    summary["runner_profile"] = corpus_gate.require_optimized(retained)
    captures = Path(tempfile.mkdtemp(prefix="artifacts-", dir=output))
    summary["artifact_directory"] = str(captures)

    def corpus(name, fixtures, both=False):
        report_path = output / f"{name}.json"
        arguments = [sys.executable, "tools/corpus_gate.py", "--runner", str(retained),
                     "--filter", ",".join(fixtures), "--json", str(report_path),
                     "--golden", str(observation_golden(
                         output, both=both, target=summary["target"]))]
        if both:
            arguments.append("--both")
        result = subprocess.run(arguments, cwd=ROOT, stdin=subprocess.DEVNULL,
                                env=dict(os.environ, OMEGA_CORPUS_CAPTURE_ARTIFACTS=str(captures / name)))
        if result.returncode not in (0, 1):
            raise ValueError(f"{name}: corpus gate exited {result.returncode}")
        validate_report(json.loads(report_path.read_text(encoding="utf-8")), fixtures,
                        both=both, revision=revision, target=summary["target"])

    stage("checking and rejection controls", lambda: corpus("checking", positives + checking_only + negatives))
    stage("native and Terminal execution", lambda: corpus("both", positives, both=True))
    if command(["git", "rev-parse", "HEAD"]) != revision or command(["git", "status", "--porcelain"]):
        raise ValueError("candidate changed during validation")


def run_suite(name, positives, negatives, *, checking_only=()):
    output = ROOT / "build" / name
    output.mkdir(parents=True, exist_ok=True)
    summary = dict(schema="omega-compiler-pilot-v1", passed=False)
    try:
        execute(output, summary, positives=positives, negatives=negatives,
                checking_only=checking_only)
        summary["passed"] = True
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError,
            corpus_gate.InfrastructureFailure) as error:
        summary["error"] = str(error)
        print(f"compiler-pilot: {error}", file=sys.stderr)
        return 1
    finally:
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")


def main():
    return run_suite("compiler-pilot", POSITIVES, NEGATIVES)


if __name__ == "__main__":
    sys.exit(main())
