#!/usr/bin/env python3
"""Build one candidate and require real compiler smoke outcomes for the queue pilot.

This deliberately small gate proves CI plumbing, not release qualification.
The corpus owns source checking, native/Terminal execution and diagnostic oracles;
this caller additionally refuses missing rows and success without execution.
"""

import argparse
from copy import deepcopy
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
from typing import Optional

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
import corpus_gate
from sample_projects import discover as discover_samples
if __package__:
    from . import compiler_replay_plan, ordinary_cli_fixture
    from . import compiler_source_export
else:
    import compiler_replay_plan
    import compiler_source_export
    import ordinary_cli_fixture

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
CLI_FIXTURE_ENVIRONMENT = "OMEGA_COMPILER_PILOT_CLI_FIXTURES"


def select_fixtures(request):
    """Validate exact existing corpus IDs before any producer is built.

    The corpus accepts substring filters. Refuse collisions rather than run an
    unrequested fixture, and never turn an invalid request into the smoke set.
    """
    if not isinstance(request, str):
        raise ValueError("fixtures must be a comma-separated string")
    if request.startswith("@"):
        plan = compiler_replay_plan.load(ROOT, request)
        return select_fixtures(",".join(plan.fixtures)) if plan.fixtures else ((), ())
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


def select_cli_fixtures(request):
    """Existing normal-exit projects, selected exactly before producer creation."""
    if not isinstance(request, str):
        raise ValueError("ordinary CLI projects must be a comma-separated string")
    selected = tuple(part.strip() for part in request.split(","))
    if not all(selected) or len(set(selected)) != len(selected):
        raise ValueError("ordinary CLI projects must be nonempty and distinct")
    corpus = tuple(project for project in selected if not project.startswith("samples/"))
    if corpus:
        _, negatives = select_fixtures(",".join(corpus))
        if negatives:
            raise ValueError("ordinary CLI fixtures must be pass/... projects")
    for fixture in selected:
        ordinary_cli_fixture.expected_exit(ROOT, fixture, corpus_gate.host_target())
    return selected


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


def authored_trap_matches(row, target):
    """The corpus owns target trap presentation and typed Terminal outcomes."""
    corpus = ROOT / "tests/omega"
    oracle = corpus_gate.native_exit_oracle(corpus / row["fixture"], target)
    try:
        if oracle.read_text(encoding="utf-8").strip() != "trap":
            return False
    except (OSError, UnicodeError):
        return False
    facts = row.get("facts", {})
    return (row["status"] == "built" and facts.get("interpreter") == "trapped"
            and "interpreter-exit" not in facts and "interpreter-signal" not in facts
            and not corpus_gate.native_exit_failures([row], corpus, target)
            and not corpus_gate.interpreter_exit_failures([row], corpus, target))


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
        elif (both or "check-target" in row.get("facts", {})) and corpus_gate.native_rejection_oracle(
                ROOT / "tests/omega" / row["fixture"],
                target if both else row["facts"]["check-target"]).is_file():
            # A product can check successfully and still owe a target refusal.
            # Authenticate its authored reason without calling it an execution.
            observed = deepcopy(row)
            failures = corpus_gate.native_rejection_failures(
                [observed], ROOT / "tests/omega", target, native_leg=both)
            if failures or not corpus_gate.corpus_records.native_rejection_satisfied(observed):
                raise ValueError(f"{row['fixture']}: missing authored native publication refusal")
        elif both:
            facts = row.get("facts", {})
            exits_agree = (facts.get("interpreter") == "agrees"
                           and isinstance(facts.get("exit"), str)
                           and re.fullmatch(r"-?\d+", facts["exit"])
                           and facts["exit"] == facts.get("interpreter-exit"))
            if (row["status"] != "built"
                    or not (exits_agree or authored_trap_matches(row, target))):
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
        recorded = output / ("recorded-" + name)
        shutil.copyfile(published, recorded)
        return recorded
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
    """Retained optimized executables required by independent consumers.

    Source and executable identities are checked before and after consumers;
    selecting another cohort never starts another Cargo build or overwrites a
    historical producer. The source revision stays the one actually built.
    """
    revision: str
    tree: str
    target: str
    toolchain: str
    cli: Path
    runner: Optional[Path]
    cli_profile: dict
    runner_profile: Optional[dict]
    consumer_revision: Optional[str] = None
    compiler_inputs: Optional[dict] = None
    replay_origin: Optional[dict] = None

    def _artifacts(self):
        if (self.runner is None) != (self.runner_profile is None):
            raise ValueError("retained corpus runner has incomplete profile custody")
        cli = ((self.cli, self.cli_profile),)
        return cli if self.runner is None else cli + ((self.runner, self.runner_profile),)

    def verify(self):
        consumer = self.consumer_revision or self.revision
        expected_tree = (self.tree if self.consumer_revision is None else
                         command(["git", "rev-parse", consumer + "^{tree}"]))
        if (command(["git", "rev-parse", "HEAD"]) != consumer
                or command(["git", "rev-parse", "HEAD^{tree}"]) != expected_tree
                or command(["git", "status", "--porcelain"])
                or os.environ.get("GITHUB_SHA", consumer) != consumer):
            raise ValueError("producer source revision, tree or working inputs changed")
        if self.consumer_revision is not None:
            compiler_replay_plan.check_inputs(ROOT, consumer, self.compiler_inputs)
        if corpus_gate.host_target() != self.target or command(["rustc", "-Vv"]) != self.toolchain:
            raise ValueError("producer host or Rust toolchain changed")
        for binary, expected in self._artifacts():
            if corpus_gate.require_optimized(binary) != expected:
                raise ValueError(f"retained producer identity/profile changed: {binary}")

    def record(self):
        record = dict(revision=self.revision, tree=self.tree, target=self.target, toolchain=self.toolchain,
                    cli=str(self.cli), runner=None if self.runner is None else str(self.runner),
                    cli_profile=self.cli_profile, runner_profile=self.runner_profile)
        if self.consumer_revision is not None:
            record.update(consumer_revision=self.consumer_revision, compiler_inputs=self.compiler_inputs,
                          replay_origin=self.replay_origin)
        return record


def retained_producer(output, plan):
    """Authenticate the restored cache without invoking a compiler build."""
    expected = plan.producer
    if command(["git", "status", "--porcelain"]):
        raise ValueError("retained producer requires clean consumer inputs")
    if corpus_gate.host_target() != expected["target"] or command(["rustc", "-Vv"]) != expected["toolchain"]:
        raise ValueError("retained producer belongs to another host or toolchain")
    revision = command(["git", "rev-parse", "HEAD"])
    paths, profiles = compiler_replay_plan.materialize(ROOT, output, plan, corpus_gate.require_optimized)
    producer = CompilerProducer(
        expected["revision"], expected["tree"], expected["target"], expected["toolchain"],
        paths["cli"], paths["runner"], profiles["cli"], profiles["runner"], revision,
        expected["input_objects"], dict(run=expected["origin_run"], cache_key=expected["cache_key"],
                                       plan=plan.path, transport="existing workflow cache"))
    producer.verify()
    (output / "retained-producer.json").write_text(json.dumps(producer.record(), indent=2) + "\n", encoding="utf-8")
    return producer


def write_replay_cache_key(request, output):
    """Restore the requested compiler, rather than the newest branch cache."""
    plan = compiler_replay_plan.load(ROOT, request)
    with Path(output).open("a", encoding="utf-8") as destination:
        destination.write("key=" + plan.producer["cache_key"] + "\n")


def write_new_record(path, record):
    """Publish a completed preparation record without replacing prior evidence."""
    with Path(path).open("x", encoding="utf-8") as stream:
        stream.write(json.dumps(record, indent=2) + "\n")


def load_producer(path):
    """Authenticate a prepared producer; failure never starts another build."""
    manifest = json.loads(Path(path).read_text(encoding="utf-8"))
    if (not isinstance(manifest, dict) or set(manifest) != {"schema", "producer"}
            or manifest["schema"] != "omega-compiler-producer-v1"):
        raise ValueError("unrecognized prepared compiler manifest")
    record = manifest["producer"]
    fields = {"revision", "tree", "target", "toolchain", "cli", "runner",
              "cli_profile", "runner_profile"}
    replay_fields = {"consumer_revision", "compiler_inputs", "replay_origin"}
    if not isinstance(record, dict) or set(record) not in (fields, fields | replay_fields):
        raise ValueError("prepared compiler manifest has incomplete producer identity")
    replay = None
    if set(record) == fields | replay_fields:
        origin = record["replay_origin"]
        if (not isinstance(record["consumer_revision"], str)
                or not re.fullmatch(r"[0-9a-f]{40}", record["consumer_revision"])
                or not isinstance(origin, dict) or set(origin) != {"run", "cache_key", "plan", "transport"}
                or type(origin["run"]) is not int or origin["run"] <= 0
                or not isinstance(origin["cache_key"], str) or not origin["cache_key"]
                or not isinstance(origin["plan"], str) or origin["transport"] != "existing workflow cache"):
            raise ValueError("prepared replay has incomplete original-source custody")
        replay = compiler_replay_plan.load(ROOT, "@" + origin["plan"])
        if (origin["run"] != replay.producer["origin_run"] or origin["cache_key"] != replay.producer["cache_key"]
                or record["compiler_inputs"] != replay.producer["input_objects"]):
            raise ValueError("prepared replay differs from its authored producer plan")
    if any(not isinstance(record[field], str) or not record[field]
           for field in ("revision", "tree", "target", "toolchain", "cli")):
        raise ValueError("prepared compiler identity fields must be nonempty strings")
    runner = record["runner"]
    if runner is not None and (not isinstance(runner, str) or not runner):
        raise ValueError("prepared corpus runner path is invalid")
    cli = Path(record["cli"])
    runner = Path(runner) if runner is not None else None
    if not cli.is_absolute() or (runner is not None and not runner.is_absolute()):
        raise ValueError("prepared executable paths must be absolute")
    producer = CompilerProducer(record["revision"], record["tree"], record["target"],
                                record["toolchain"], cli, runner,
                                record["cli_profile"], record["runner_profile"],
                                record.get("consumer_revision"), record.get("compiler_inputs"),
                                record.get("replay_origin"))
    for _, profile in producer._artifacts():
        if (not isinstance(profile, dict) or not isinstance(profile.get("sha256"), str)
                or not re.fullmatch(r"[0-9a-f]{64}", profile["sha256"])):
            raise ValueError("prepared executable lacks its verified digest/profile")
    if replay is not None:
        require_plan_producer(producer, replay)
    producer.verify()
    return producer


def require_plan_producer(producer, plan):
    """A selected replay cannot silently use another prepared compiler."""
    expected = plan.producer
    if any(getattr(producer, key) != expected[key] for key in ("revision", "tree", "target", "toolchain")):
        raise ValueError("prepared compiler differs from the selected replay producer")
    for kind in ("cli", "runner"):
        profile = getattr(producer, kind + "_profile")
        identity = None if profile is None else {key: profile.get(key) for key in ("sha256", "profile")}
        if identity != expected["executables"][kind]:
            raise ValueError("prepared executable differs from the selected replay producer")


def prepare_producer(output, *, need_runner=True):
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
    # Retain diagnostics while the compiler runs. A failed build can exhaust
    # storage or be interrupted before a later buffered-output write succeeds.
    stdout_path = retained / "cli-build.jsonl"
    stderr_path = retained / "cli-build.stderr"
    with stdout_path.open("w", encoding="utf-8") as stdout, \
            stderr_path.open("w", encoding="utf-8") as stderr, build_slot.hold(ROOT):
        result = subprocess.run(arguments, cwd=ROOT, stdin=subprocess.DEVNULL,
                                stdout=stdout, stderr=stderr, text=True,
                                env=corpus_gate.optimized_environment())
    if result.returncode:
        sys.stderr.write(stderr_path.read_text(encoding="utf-8")[-8000:])
        raise ValueError(f"CLI build failed ({result.returncode}); raw build receipt retained")
    artifacts = [json.loads(line) for line in stdout_path.read_text(encoding="utf-8").splitlines()
                 if line.strip().startswith("{")]
    binaries = [row["executable"] for row in artifacts if row.get("reason") == "compiler-artifact"
                and row.get("target", {}).get("name") == "omega" and row.get("executable")]
    if not binaries or not artifacts[-1].get("success"):
        raise ValueError("CLI build supplied no completed executable artifact")
    cli = Path(binaries[-1])
    # Current CLIs report their own build-time profile. Cargo fingerprint
    # reconstruction belongs to historical binaries without that protocol;
    # an internal Cargo output layout is not the CLI's profile authority.
    cli_profile = corpus_gate.require_optimized(cli)
    name = "omega.exe" if os.name == "nt" else "omega"
    copied_cli = retained / name
    shutil.copy2(cli, copied_cli)
    copied_runner = None
    runner_profile = None
    if need_runner:
        runner = corpus_gate.build_runner()
        copied_runner = retained / ("corpus_runner.exe" if runner.suffix == ".exe" else "corpus_runner")
        shutil.copy2(runner, copied_runner)
        shutil.copy2(str(runner) + ".compiler-profile.json", str(copied_runner) + ".compiler-profile.json")
        runner_profile = corpus_gate.require_optimized(copied_runner)
    producer = CompilerProducer(revision, command(["git", "rev-parse", "HEAD^{tree}"]),
                                corpus_gate.host_target(), command(["rustc", "-Vv"]), copied_cli.resolve(),
                                copied_runner.resolve() if copied_runner is not None else None,
                                corpus_gate.require_optimized(copied_cli), runner_profile)
    if producer.cli_profile["sha256"] != cli_profile["sha256"] or corpus_gate.require_optimized(cli) != cli_profile:
        raise ValueError("CLI changed between its completed build and retained producer")
    producer.verify()
    for binary, profile in producer._artifacts():
        Path(str(binary) + ".verified-profile.json").write_text(json.dumps(dict(
            source_revision=revision, source_tree=producer.tree, toolchain=producer.toolchain,
            observation=profile), indent=2) + "\n", encoding="utf-8")
    if command(["git", "status", "--porcelain"]):
        raise ValueError("source changed while preparing producer")
    (retained / "producer.json").write_text(json.dumps(producer.record(), indent=2) + "\n", encoding="utf-8")
    return producer


def execute(output, summary, *, positives=POSITIVES, negatives=NEGATIVES, checking_only=(), producer=None, samples=(), cli_fixtures=()):
    if not (positives or negatives or checking_only or samples or cli_fixtures):
        raise ValueError("pilot requires at least one fixture")
    revision = command(["git", "rev-parse", "HEAD"])
    if command(["git", "status", "--porcelain"]):
        raise ValueError("pilot requires a clean candidate")
    if os.environ.get("GITHUB_SHA", revision) != revision:
        raise ValueError("checkout is not the GitHub candidate being checked")
    summary.update(candidate=revision, target=corpus_gate.host_target(),
                   toolchain=command(["rustc", "-Vv"]), stages=[],
                   fixtures=dict(positive=positives, negative=negatives,
                                 checking_only=checking_only), samples=samples, cli_fixtures=cli_fixtures)

    def stage(name, operation):
        started = time.monotonic()
        result = dict(name=name, passed=False)
        summary["stages"].append(result)
        print(f"compiler-pilot: {name}", flush=True)
        try:
            value = operation()
            result["passed"] = True
            return value
        except (OSError, ValueError, subprocess.CalledProcessError,
                corpus_gate.InfrastructureFailure) as error:
            result["error"] = str(error)
            raise
        finally:
            result["seconds"] = round(time.monotonic() - started, 3)

    need_runner = bool(positives or negatives or checking_only)
    revisions = (revision,) if producer is None else (revision, producer.revision)
    summary["source_exports"] = stage("retain measured source", lambda: compiler_source_export.export(output, revisions))
    if producer is None:
        producer = stage("prepare optimized compiler", lambda: prepare_producer(output, need_runner=need_runner))
    stage("authenticate optimized producer", producer.verify)
    if (producer.consumer_revision or producer.revision) != revision or producer.target != summary["target"]:
        raise ValueError("producer belongs to another source or host")
    summary["producer"] = producer.record()
    if need_runner and (producer.runner is None or producer.runner_profile is None):
        raise ValueError("fixture execution requires a retained optimized corpus runner")
    if producer.runner_profile is not None:
        summary["runner_sha256"] = producer.runner_profile["sha256"]
        summary["runner_profile"] = producer.runner_profile
    retained = producer.runner
    captures = Path(tempfile.mkdtemp(prefix="artifacts-", dir=output))
    summary["artifact_directory"] = str(captures)

    def corpus(name, fixtures, both=False):
        report_path = output / f"{name}.json"
        arguments = [sys.executable, "tools/corpus_gate.py", "--runner", str(retained),
                     "--record", "--filter", ",".join(fixtures), "--json", str(report_path),
                     "--golden", str(observation_golden(
                         output, both=both, target=summary["target"]))]
        if both:
            arguments.append("--both")
        result = subprocess.run(arguments, cwd=ROOT, stdin=subprocess.DEVNULL,
                                env=dict(os.environ, OMEGA_CORPUS_SWEEP="1",
                                         OMEGA_CORPUS_CAPTURE_ARTIFACTS=str(captures / name)))
        if result.returncode not in (0, 1):
            raise ValueError(f"{name}: corpus gate exited {result.returncode}")
        validate_report(json.loads(report_path.read_text(encoding="utf-8")), fixtures,
                        both=both, revision=revision, target=summary["target"])

    refusals = []

    def observe(name, operation):
        try:
            stage(name, operation)
        except (OSError, ValueError, subprocess.CalledProcessError,
                corpus_gate.InfrastructureFailure) as error:
            # Independent consumers can still produce useful evidence from
            # this compiler. Keep every failure and leave the run red.
            refusals.append(f"{name}: {error}")
            summary["consumer_errors"] = list(refusals)
        # A failed fixture does not invalidate the compiler. Changed compiler
        # bytes do: stop before another consumer uses an unauthenticated file.
        producer.verify()

    if positives or checking_only or negatives:
        checking = tuple(dict.fromkeys(positives + checking_only + negatives))
        observe("checking and rejection controls", lambda: corpus("checking", checking))
    if positives:
        observe("native and Terminal execution", lambda: corpus("both", positives, both=True))
    if cli_fixtures:
        def ordinary_cli():
            rows = []
            errors = []
            summary["cli_observations"] = rows
            for fixture in cli_fixtures:
                row = ordinary_cli_fixture.observe(ROOT, producer, fixture, output / "ordinary-cli" / fixture)
                compact = {key: value for key, value in row.items() if key not in ("inputs", "producer")}
                compact["input_count"] = len(row.get("inputs", {}))
                compact["input_manifest_sha256"] = hashlib.sha256(
                    json.dumps(row.get("inputs", {}), sort_keys=True).encode("utf-8")).hexdigest()
                compact["full_observation"] = str(output / "ordinary-cli" / fixture / "observation.json")
                rows.append(compact)
                (output / "cli.json").write_text(json.dumps(rows, indent=2) + "\n", encoding="utf-8")
                producer.verify()
                if row.get("passed") is not True:
                    errors.append(f"{fixture}: {row.get('error', 'ordinary CLI observation failed')}")
            if errors:
                raise ValueError("; ".join(errors))
        observe("ordinary CLI package admission and native/Terminal execution", ordinary_cli)
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
        observe("fact sample native and Terminal execution", fact_samples)
    producer.verify()
    if command(["git", "rev-parse", "HEAD"]) != revision or command(["git", "status", "--porcelain"]):
        raise ValueError("candidate changed during validation")
    if refusals:
        raise ValueError("; ".join(refusals))


def selected_consumers(positives, negatives, checking_only, *, fixtures, samples, cli_fixtures, check_only):
    """Resolve the complete request before either preparing or using a compiler."""
    replay = None
    if isinstance(fixtures, str) and fixtures.startswith("@"):
        if samples is not None or cli_fixtures is not None or check_only:
            raise ValueError("a replay plan owns its producer and complete consumer selection")
        replay = compiler_replay_plan.load(ROOT, fixtures)
        fixtures = ",".join(replay.fixtures) if replay.fixtures else None
        samples = ",".join(replay.samples) if replay.samples else None
        cli_fixtures = ",".join(replay.cli_fixtures) if replay.cli_fixtures else None
        positives, negatives, checking_only = (), (), ()
    if check_only and (fixtures is None or samples is not None or cli_fixtures is not None):
        raise ValueError("checking-only requires exact fixtures without samples or ordinary CLI projects")
    selected_samples = select_samples(samples) if samples is not None else ()
    selected_cli = select_cli_fixtures(cli_fixtures) if cli_fixtures is not None else ()
    if (samples is not None or cli_fixtures is not None) and fixtures is None:
        positives, negatives, checking_only = (), (), ()
    if fixtures is not None:
        positives, negatives = select_fixtures(fixtures)
        checking_only = ()
    if check_only:
        checking_only, positives = positives, ()
    if not (positives or negatives or checking_only or selected_samples or selected_cli):
        raise ValueError("pilot requires at least one fixture")
    if replay is not None and replay.source_only:
        checking_only, positives = positives, ()
    return (dict(positives=positives, negatives=negatives, checking_only=checking_only,
                 samples=selected_samples, cli_fixtures=selected_cli), replay)


def run_suite(name, positives, negatives, *, checking_only=(), producer=None, fixtures=None, samples=None, cli_fixtures=None, check_only=False, phase="all"):
    output = ROOT / "build" / name
    output.mkdir(parents=True, exist_ok=True)
    preparing = phase == "prepare"
    report = output / ("preparation.json" if preparing else "summary.json")
    manifest = output / "producer.json"
    # A second preparation must not replace either the compiler association or
    # its result, including a prior failed attempt. Use another output directory.
    if preparing and (report.exists() or manifest.exists()):
        print("compiler-pilot: preparation records already exist", file=sys.stderr)
        return 1
    summary = dict(schema="omega-compiler-preparation-v1" if preparing else "omega-compiler-pilot-v1",
                   passed=False)
    started = time.monotonic()
    try:
        if phase not in ("all", "prepare", "consume") or (phase != "all" and producer is not None):
            raise ValueError("separate phases require their recorded producer, not an in-memory override")
        selected, replay = selected_consumers(positives, negatives, checking_only, fixtures=fixtures,
                                             samples=samples, cli_fixtures=cli_fixtures, check_only=check_only)
        if replay is not None:
            if producer is not None:
                raise ValueError("a replay plan owns its producer and complete consumer selection")
            summary["replay_plan"] = replay.path
        if preparing:
            revision = command(["git", "rev-parse", "HEAD"])
            summary.update(candidate=revision, tree=command(["git", "rev-parse", "HEAD^{tree}"]),
                           target=corpus_gate.host_target(), toolchain=command(["rustc", "-Vv"]),
                           consumers=selected)
            revisions = (revision,) if replay is None else (revision, replay.producer["revision"])
            summary["source_exports"] = compiler_source_export.export(output, revisions)
            producer = (prepare_producer(output, need_runner=bool(
                selected["positives"] or selected["negatives"] or selected["checking_only"]))
                if replay is None else retained_producer(output, replay))
            write_new_record(manifest, dict(schema="omega-compiler-producer-v1", producer=producer.record()))
            summary["producer"] = producer.record()
        else:
            if phase == "consume":
                producer = load_producer(manifest)
                if replay is not None:
                    require_plan_producer(producer, replay)
            elif replay is not None:
                producer = retained_producer(output, replay)
            execute(output, summary, producer=producer, **selected)
        summary["passed"] = True
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError,
            corpus_gate.InfrastructureFailure) as error:
        summary["error"] = str(error)
        print(f"compiler-pilot: {error}", file=sys.stderr)
        return 1
    finally:
        if preparing:
            summary["seconds"] = round(time.monotonic() - started, 3)
            write_new_record(report, summary)
        else:
            report.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=("all", "prepare", "consume"), default="all",
                        help="prepare and retain the compiler, consume its manifest without building, or do both")
    parser.add_argument("--fixtures", help="exact pass/... or fail/... IDs, or @tracked/replay-plan.json")
    parser.add_argument("--samples", help="comma-separated exact samples/cli/facts project paths (includes descendants)")
    parser.add_argument("--cli-fixtures", help="exact pass/... normal-exit projects through ordinary CLI review/run")
    parser.add_argument("--checking-only", action="store_true",
                        default=os.environ.get("OMEGA_COMPILER_PILOT_CHECKING_ONLY") == "true",
                        help="check exact --fixtures and rejection controls without executing positives")
    options = parser.parse_args(arguments)
    fixtures = options.fixtures
    if fixtures is None:
        fixtures = os.environ.get(FIXTURE_ENVIRONMENT) or None
    samples = options.samples if options.samples is not None else os.environ.get(SAMPLE_ENVIRONMENT) or None
    cli_fixtures = (options.cli_fixtures if options.cli_fixtures is not None
                    else os.environ.get(CLI_FIXTURE_ENVIRONMENT) or None)
    return run_suite("compiler-pilot", POSITIVES, NEGATIVES, fixtures=fixtures,
                     samples=samples, cli_fixtures=cli_fixtures, check_only=options.checking_only,
                     phase=options.phase)


if __name__ == "__main__":
    sys.exit(main())
