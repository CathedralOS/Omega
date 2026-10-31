#!/usr/bin/env python3
"""Build one candidate and require real compiler smoke outcomes for the queue pilot.

This deliberately small gate proves CI plumbing, not release qualification.
The corpus owns source checking, native/Terminal execution and diagnostic oracles;
this caller additionally refuses missing rows and success without execution.
"""

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import corpus_gate
from sample_projects import discover as discover_samples

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
FIXTURE_ENVIRONMENT = "OMEGA_COMPILER_PILOT_FIXTURES"
SAMPLE_ENVIRONMENT = "OMEGA_COMPILER_PILOT_SAMPLES"


def select_fixtures(request):
    """Validate exact existing corpus IDs before any producer is built.

    The corpus accepts substring filters. Refuse collisions rather than run an
    unrequested fixture, and never turn an invalid request into the smoke set.
    """
    if not isinstance(request, str):
        raise ValueError("fixtures must be a comma-separated string")
    fixtures = tuple(part.strip() for part in request.split(","))
    if not fixtures or any(not fixture for fixture in fixtures):
        raise ValueError("fixtures must be nonempty comma-separated corpus IDs")
    if len(set(fixtures)) != len(fixtures):
        raise ValueError("fixture IDs must be distinct")
    root = (ROOT / "tests" / "omega").resolve()
    groups = set()
    for fixture in fixtures:
        path = PurePosixPath(fixture)
        if (len(path.parts) < 3 or path.parts[0] not in ("pass", "fail")
                or any(part in (".", "..") for part in path.parts)
                or "\\" in fixture or path.as_posix() != fixture):
            raise ValueError(f"not a normalized pass/... or fail/... fixture ID: {fixture!r}")
        directory = root.joinpath(*path.parts)
        if not directory.resolve().is_relative_to(root) or not (directory / "main.omg").is_file():
            raise ValueError(f"fixture has no existing main.omg inside tests/omega: {fixture}")
        groups.add(directory.parent)
    matched = {directory.relative_to(root).as_posix()
               for group in groups for directory in group.iterdir()
               if (directory / "main.omg").is_file()
               and any(fragment in directory.relative_to(root).as_posix() for fragment in fixtures)}
    if matched != set(fixtures):
        raise ValueError("corpus substring filter would also select unrequested fixtures: "
                         + ", ".join(sorted(matched - set(fixtures))))
    return (tuple(fixture for fixture in fixtures if fixture.startswith("pass/")),
            tuple(fixture for fixture in fixtures if fixture.startswith("fail/")))


def select_samples(request):
    """Select existing fact projects and descendants, refusing filter collisions."""
    if not isinstance(request, str):
        raise ValueError("samples must be a comma-separated string")
    roots = tuple(part.strip() for part in request.split(","))
    if not all(roots) or len(set(roots)) != len(roots):
        raise ValueError("samples must be nonempty distinct fact-project paths")
    facts = (ROOT / "samples/cli/facts").resolve()
    for sample in roots:
        path = PurePosixPath(sample)
        if (len(path.parts) < 4 or path.parts[:3] != ("samples", "cli", "facts")
                or path.as_posix() != sample or "\\" in sample
                or any(part in (".", "..") for part in path.parts)
                or not (ROOT / sample).resolve().is_relative_to(facts)
                or not (ROOT / sample / "build.omg").is_file()):
            raise ValueError(f"not an existing normalized fact-project path: {sample!r}")
    projects = discover_samples(facts, ROOT, roots)
    selected = tuple(project.directory.relative_to(ROOT.resolve()).as_posix()
                     for project in projects if project.role != "library")
    if (not set(roots).issubset(selected) or any(project.entry is None for project in projects)
            or any(not any(sample == root or sample.startswith(root + "/") for root in roots)
                   for sample in selected)):
        raise ValueError("sample filter selects missing roots, entries, or unrequested projects")
    return selected


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
                    or not isinstance(facts.get("exit"), str)
                    or not re.fullmatch(r"-?\d+", facts.get("exit", ""))
                    or facts["exit"] != facts.get("interpreter-exit")):
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


@dataclass(frozen=True)
class CompilerProducer:
    """One retained optimized CLI/runner pair shared by independent cohorts.

    Source and executable identities are checked before and after consumers;
    selecting another cohort never starts another Cargo build or overwrites a
    historical producer. The source revision stays the one actually built.
    """
    revision: str
    tree: str
    target: str
    toolchain: str
    cli: Path
    runner: Path
    cli_profile: dict
    runner_profile: dict

    def verify(self):
        if command(["git", "rev-parse", "HEAD"]) != self.revision or command(["git", "status", "--porcelain"]):
            raise ValueError("producer source revision or working inputs changed")
        for binary, expected in ((self.cli, self.cli_profile), (self.runner, self.runner_profile)):
            if corpus_gate.require_optimized(binary) != expected:
                raise ValueError(f"retained producer identity/profile changed: {binary}")

    def record(self):
        return dict(revision=self.revision, tree=self.tree, target=self.target, toolchain=self.toolchain,
                    cli=str(self.cli), runner=str(self.runner),
                    cli_profile=self.cli_profile, runner_profile=self.runner_profile)


def prepare_producer(output):
    """Build and retain the ordinary producer once for validated consumers."""
    output = Path(output).resolve()
    revision = command(["git", "rev-parse", "HEAD"])
    if command(["git", "status", "--porcelain"]) or os.environ.get("GITHUB_SHA", revision) != revision:
        raise ValueError("producer requires the clean exact candidate")
    output.mkdir(parents=True, exist_ok=True)
    retained = Path(tempfile.mkdtemp(prefix="producer-", dir=output))
    tool = "mbx" if shutil.which("mbx") else "cargo"
    arguments = [tool, "build", "--locked", "-p", "omega", "--message-format=json"]
    import build_slot
    with build_slot.hold(ROOT):
        result = subprocess.run(arguments, cwd=ROOT, stdin=subprocess.DEVNULL,
                                capture_output=True, text=True, env=corpus_gate.optimized_environment())
    (retained / "cli-build.jsonl").write_text(result.stdout, encoding="utf-8")
    (retained / "cli-build.stderr").write_text(result.stderr, encoding="utf-8")
    if result.returncode:
        sys.stderr.write(result.stderr[-8000:])
        raise ValueError(f"CLI build failed ({result.returncode}); raw build receipt retained")
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.strip().startswith("{")]
    binaries = [row["executable"] for row in artifacts if row.get("reason") == "compiler-artifact"
                and row.get("target", {}).get("name") == "omega" and row.get("executable")]
    if not binaries or not artifacts[-1].get("success"):
        raise ValueError("CLI build supplied no completed executable artifact")
    cli = Path(binaries[-1])
    # Current CLIs report their own build-time profile. Cargo fingerprint
    # reconstruction belongs to historical binaries without that protocol;
    # an internal Cargo output layout is not the CLI's profile authority.
    cli_profile = corpus_gate.require_optimized(cli)
    runner = corpus_gate.build_runner()
    name = "omega.exe" if os.name == "nt" else "omega"
    copied_cli = retained / name
    copied_runner = retained / ("corpus_runner.exe" if runner.suffix == ".exe" else "corpus_runner")
    shutil.copy2(cli, copied_cli)
    shutil.copy2(runner, copied_runner)
    shutil.copy2(str(runner) + ".compiler-profile.json", str(copied_runner) + ".compiler-profile.json")
    producer = CompilerProducer(revision, command(["git", "rev-parse", "HEAD^{tree}"]),
                                corpus_gate.host_target(), command(["rustc", "-Vv"]), copied_cli.resolve(),
                                copied_runner.resolve(), corpus_gate.require_optimized(copied_cli),
                                corpus_gate.require_optimized(copied_runner))
    if producer.cli_profile["sha256"] != cli_profile["sha256"] or corpus_gate.require_optimized(cli) != cli_profile:
        raise ValueError("CLI changed between its completed build and retained producer")
    producer.verify()
    for binary, profile in ((producer.cli, producer.cli_profile), (producer.runner, producer.runner_profile)):
        Path(str(binary) + ".verified-profile.json").write_text(json.dumps(dict(
            source_revision=revision, source_tree=producer.tree, toolchain=producer.toolchain,
            observation=profile), indent=2) + "\n", encoding="utf-8")
    if command(["git", "status", "--porcelain"]):
        raise ValueError("source changed while preparing producer")
    (retained / "producer.json").write_text(json.dumps(producer.record(), indent=2) + "\n", encoding="utf-8")
    return producer


def execute(output, summary, *, positives=POSITIVES, negatives=NEGATIVES, checking_only=(), producer=None, samples=()):
    if not (positives or negatives or checking_only or samples):
        raise ValueError("pilot requires at least one fixture")
    revision = command(["git", "rev-parse", "HEAD"])
    if command(["git", "status", "--porcelain"]):
        raise ValueError("pilot requires a clean candidate")
    if os.environ.get("GITHUB_SHA", revision) != revision:
        raise ValueError("checkout is not the GitHub candidate being checked")
    summary.update(candidate=revision, target=corpus_gate.host_target(),
                   toolchain=command(["rustc", "-Vv"]), stages=[],
                   fixtures=dict(positive=positives, negative=negatives,
                                 checking_only=checking_only), samples=samples)

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

    if producer is None:
        producer = stage("prepare optimized compiler", lambda: prepare_producer(output))
    stage("authenticate optimized producer", producer.verify)
    if producer.revision != revision or producer.target != summary["target"]:
        raise ValueError("producer belongs to another source or host")
    summary["producer"] = producer.record()
    summary["runner_sha256"] = producer.runner_profile["sha256"]
    summary["runner_profile"] = producer.runner_profile
    retained = producer.runner
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

    if positives or checking_only or negatives:
        stage("checking and rejection controls", lambda: corpus("checking", positives + checking_only + negatives))
    if positives:
        stage("native and Terminal execution", lambda: corpus("both", positives, both=True))
    if samples:
        def fact_samples():
            report = output / "samples.json"
            result = subprocess.run([sys.executable, "tools/sample_native_gate.py", "--cli", str(producer.cli),
                                     "--facts", "--jobs", "2", "--target", producer.target, "--filter", ",".join(samples),
                                     "--write-results", str(report)], cwd=ROOT, stdin=subprocess.DEVNULL)
            rows = json.loads(report.read_text(encoding="utf-8"))
            summary["sample_observations"] = rows
            if (result.returncode != 0 or not rows or len(rows) != len(samples)
                    or {row["sample"] for row in rows} != set(samples)
                    or any(row.get("state") not in ("pass", "rejected") for row in rows)):
                raise ValueError("fact samples lack complete runtime/oracle/rejection evidence")
        stage("fact sample native and Terminal execution", fact_samples)
    producer.verify()
    if command(["git", "rev-parse", "HEAD"]) != revision or command(["git", "status", "--porcelain"]):
        raise ValueError("candidate changed during validation")


def run_suite(name, positives, negatives, *, checking_only=(), producer=None, fixtures=None, samples=None):
    output = ROOT / "build" / name
    output.mkdir(parents=True, exist_ok=True)
    summary = dict(schema="omega-compiler-pilot-v1", passed=False)
    try:
        selected_samples = select_samples(samples) if samples is not None else ()
        if samples is not None and fixtures is None:
            positives, negatives, checking_only = (), (), ()
        if fixtures is not None:
            positives, negatives = select_fixtures(fixtures)
            checking_only = ()
        execute(output, summary, positives=positives, negatives=negatives,
                checking_only=checking_only, producer=producer, samples=selected_samples)
        summary["passed"] = True
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError,
            corpus_gate.InfrastructureFailure) as error:
        summary["error"] = str(error)
        print(f"compiler-pilot: {error}", file=sys.stderr)
        return 1
    finally:
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", help="comma-separated exact pass/... or fail/... fixture IDs")
    parser.add_argument("--samples", help="comma-separated exact samples/cli/facts project paths (includes descendants)")
    options = parser.parse_args(arguments)
    fixtures = options.fixtures
    if fixtures is None:
        fixtures = os.environ.get(FIXTURE_ENVIRONMENT) or None
    samples = options.samples if options.samples is not None else os.environ.get(SAMPLE_ENVIRONMENT) or None
    return run_suite("compiler-pilot", POSITIVES, NEGATIVES, fixtures=fixtures, samples=samples)


if __name__ == "__main__":
    sys.exit(main())
