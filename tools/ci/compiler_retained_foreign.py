#!/usr/bin/env python3
"""Real retained C pointers in default Terminal and native execution on this host."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

import compiler_pilot

ROOT = compiler_pilot.ROOT
sys.path.insert(0, str(ROOT / "tools"))
import sample_native_gate

PLACES = ("local", "field", "parameter", "call_result", "match_join", "const_name",
          "other_state", "other_machine", "target_body")
# Authored fact path, corpus prefix, provider owner, positive controls, rejects.
FACTS = (
    ("foreign_retained_shared_borrow", "foreign_retained_shared_borrow",
     "foreign_retained_shared_borrow",
     ("shared_aliases", "resident_whole_move", "enclosing_allocation"), ()),
    ("foreign_retained_mutable_borrow", "foreign_retained_mutable_borrow",
     "foreign_retained_mutable_borrow", ("reverse_completion", "mixed_issuers"),
     ("stale_completion_fact", "other_registration_live", "overlapping_registration", "mixed_issuers_live")),
    ("foreign_nested_record_backing", "foreign_nested_record_backing",
     "foreign_nested_record_backing", ("repeated_child_type",), ("repeated_child_type_live",)),
    ("foreign_nested_record_backing/views", "foreign_borrowed_array_view",
     "foreign_nested_record_backing", (), ()),
    ("foreign_nested_record_backing/arrays", "foreign_nested_array_backing",
     "foreign_nested_record_backing", ("computed_completions",), ("invalid_const",)),
)


def samples(output, target, cli, fact, controls):
    """Replay authored sources, never copy acceptance or generated products."""
    cli = Path(cli).resolve()
    profile = sample_native_gate.require_optimized(cli)
    digest = hashlib.sha256(cli.read_bytes()).hexdigest()
    logs = output / "sample-logs"
    logs.mkdir(parents=True, exist_ok=True)
    observations = []
    report = dict(candidate=compiler_pilot.command(["git", "rev-parse", "HEAD"]),
                  compiler=str(cli), compiler_sha256=digest, compiler_profile=profile,
                  target=target, observations=observations, passed=False)
    try:
        for place in PLACES + controls:
            authored = fact / place if place in PLACES else fact / "controls" / place
            replay = output / "samples" / place
            replay.mkdir(parents=True, exist_ok=False)
            sources = {}
            for name in ("main.omg", "build.omg", "sample.json"):
                source = authored / name
                if source.is_file():
                    shutil.copy2(source, replay / name)
                    sources[name] = hashlib.sha256(source.read_bytes()).hexdigest()
            failure, review = sample_native_gate.accept_review(str(cli), str(replay), target, 180)
            (logs / (place + ".review.log")).write_text(review, encoding="utf-8")
            if failure:
                raise ValueError(f"{place}: ordinary package preparation failed: {failure}")
            # An explicit --target requests cross-target compilation in this
            # CLI; matching-host execution deliberately supplies no such flag.
            arguments = [str(cli), "run", "--both", str(replay / "main.omg")]
            started = time.monotonic()
            result = subprocess.run(arguments, cwd=ROOT, stdin=subprocess.DEVNULL,
                                    capture_output=True, text=True, timeout=180)
            (logs / (place + ".stdout.txt")).write_text(result.stdout, encoding="utf-8")
            (logs / (place + ".stderr.txt")).write_text(result.stderr, encoding="utf-8")
            observed = dict(place=place, argv=arguments, exit=result.returncode,
                            source_sha256=sources, seconds=time.monotonic() - started,
                            native_zero="native exit 0" in result.stderr.splitlines(),
                            terminal_zero="interp exit: 0" in result.stderr.splitlines())
            observations.append(observed)
            if result.returncode or not observed["native_zero"] or not observed["terminal_zero"]:
                raise ValueError(f"{place}: missing actual native/Terminal zero exits: {result.stderr}")
            print(f"retained foreign: {place}: actual native 0, Terminal 0", flush=True)
        if hashlib.sha256(cli.read_bytes()).hexdigest() != digest:
            raise ValueError("CLI producer changed during execution")
        report["passed"] = True
    finally:
        (output / "sample-observations.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


def main():
    output = ROOT / "build/compiler-retained-foreign"
    output.mkdir(parents=True, exist_ok=True)
    os.environ["OMEGA_LIBRARY_ROOT"] = str(ROOT / "source/library")
    name = "omega.exe" if os.name == "nt" else "omega"
    built = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug" / name
    observations = []
    retained = None
    for fact_name, fixture_prefix, provider_name, controls, reject_controls in FACTS:
        observation = dict(fact=fact_name, passed=False)
        observations.append(observation)
        try:
            retained = qualify_cohort(output, built, fact_name, fixture_prefix, provider_name, controls, reject_controls)
            observation["passed"] = True
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            observation["error"] = str(error)
            print(f"retained foreign: {fact_name}: {error}", file=sys.stderr)
    if retained is not None:
        try:
            unoffered_descriptor(output, compiler_pilot.corpus_gate.host_target(), retained)
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            observations.append(dict(fact="unoffered_descriptor", passed=False, error=str(error)))
    passed = all(row["passed"] for row in observations)
    (output / "summary.json").write_text(json.dumps(dict(
        candidate=compiler_pilot.command(["git", "rev-parse", "HEAD"]),
        target=compiler_pilot.corpus_gate.host_target(),
        observations=observations, passed=passed), indent=2) + "\n", encoding="utf-8")
    return 0 if passed else 1


def qualify_cohort(output, built, fact_name, fixture_prefix, provider_name, controls, reject_controls):
    """A failed cohort stays red without hiding later independent observations."""
    fact = ROOT / "samples/cli/facts" / fact_name
    cohort = output / fact_name
    cohort.mkdir(parents=True, exist_ok=True)
    provider = cohort / "provider"
    provider_source = ROOT / "samples/cli/facts" / provider_name / "fixture/prepare.py"
    prepared = subprocess.run([sys.executable, str(provider_source),
                               "--output", str(provider)], cwd=ROOT,
                              capture_output=True, text=True, timeout=180)
    (cohort / "provider.stdout.json").write_text(prepared.stdout, encoding="utf-8")
    (cohort / "provider.stderr.txt").write_text(prepared.stderr, encoding="utf-8")
    if prepared.returncode:
        raise ValueError(f"{fact_name}: matching-host C fixture failed: {prepared.stderr}")
    key = ("PATH" if os.name == "nt" else "DYLD_LIBRARY_PATH"
           if sys.platform == "darwin" else "LD_LIBRARY_PATH")
    os.environ[key] = str(provider) + (os.pathsep + os.environ[key] if os.environ.get(key) else "")
    positives = tuple(f"pass/facts/{fixture_prefix}_{place}" for place in PLACES + controls)
    negatives = tuple(f"fail/facts/{fixture_prefix}_{place}" for place in PLACES + reject_controls)
    status = compiler_pilot.run_suite("compiler-retained-foreign/" + fact_name,
                                      positives, negatives)
    if status:
        raise ValueError(f"corpus checking or native/Terminal execution failed (exit {status})")
    retained = cohort / built.name
    shutil.copy2(built, retained)
    samples(cohort, compiler_pilot.corpus_gate.host_target(), retained, fact, controls)
    if fact_name == "foreign_retained_mutable_borrow":
        uncovered_qualification(cohort, compiler_pilot.corpus_gate.host_target(), retained, fact)
    return retained


def uncovered_qualification(output, target, cli, fact):
    """A normal guarantee cannot cover an extra qualification on the caller."""
    authored = fact / "controls/uncovered_qualification"
    replay = output / "uncovered-qualification"
    replay.mkdir(exist_ok=False)
    sources = {}
    for name in ("main.omg", "build.omg"):
        shutil.copy2(authored / name, replay / name)
        sources[name] = hashlib.sha256((authored / name).read_bytes()).hexdigest()
    failure, review = sample_native_gate.accept_review(str(cli), str(replay), target, 180)
    (output / "uncovered-qualification.review.log").write_text(review, encoding="utf-8")
    if failure:
        raise ValueError(f"uncovered qualification: package preparation failed: {failure}")
    arguments = [str(cli), "run", "--both", str(replay / "main.omg")]
    result = subprocess.run(arguments, cwd=ROOT, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=180)
    (output / "uncovered-qualification.stdout.txt").write_text(result.stdout, encoding="utf-8")
    (output / "uncovered-qualification.stderr.txt").write_text(result.stderr, encoding="utf-8")
    refused = (result.returncode == 201 and "native exit 0" in result.stderr.splitlines()
               and "mutable host storage has an uncovered original qualification or owner"
               in result.stderr)
    (output / "uncovered-qualification.json").write_text(json.dumps(dict(
        argv=arguments, exit=result.returncode, target=target, source_sha256=sources,
        compiler_sha256=hashlib.sha256(Path(cli).read_bytes()).hexdigest(),
        intended_refusal=refused), indent=2) + "\n", encoding="utf-8")
    if not refused:
        raise ValueError(f"uncovered qualification: missing intended refusal: {result.stderr}")


def unoffered_descriptor(output, target, cli):
    """Actual finite input reaches native C; unavailable Terminal I/O declines."""
    if target not in ("linux_x86_64", "linux_arm64"):
        return
    authored = ROOT / "tests/omega/pass/boundary/native_provider_unoffered_read"
    replay = output / "unoffered-descriptor"
    replay.mkdir(exist_ok=False)
    for name in ("main.omg", "build.omg"):
        shutil.copy2(authored / name, replay / name)
    failure, review = sample_native_gate.accept_review(str(cli), str(replay), target, 180)
    (output / "unoffered-descriptor.review.log").write_text(review, encoding="utf-8")
    if failure:
        raise ValueError(f"unoffered descriptor: package preparation failed: {failure}")
    supplied = output / "letter.stdin"
    supplied.write_bytes(b"A")
    results = output / "descriptor-results"
    arguments = [str(cli), "run", "--both", "--input", str(supplied),
                 "--outputs", str(results), str(replay / "main.omg")]
    result = subprocess.run(arguments, cwd=ROOT, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=180)
    (output / "unoffered-descriptor.stdout.txt").write_text(result.stdout, encoding="utf-8")
    (output / "unoffered-descriptor.stderr.txt").write_text(result.stderr, encoding="utf-8")
    table = (results / "results.tsv").read_text(encoding="utf-8")
    if (result.returncode or "letter\t0\tdeclined\t" not in table
            or "descriptor operation requires its explicit modeled I/O capability" not in result.stderr):
        raise ValueError(f"unoffered descriptor: missing native zero / explicit Terminal refusal: {table} {result.stderr}")


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"retained foreign: {error}", file=sys.stderr)
        sys.exit(1)
