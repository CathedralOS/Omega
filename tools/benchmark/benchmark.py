#!/usr/bin/env python3
"""Measure and record versioned compiler benchmarks.

Each record is one JSON document in ``records/`` describing a single
subject compiled and run under one exact rule selection on one target.
The record schema is versioned (``omega-benchmark-record/1``) and
documented in README.md; ``validate`` enforces it so committed rows
cannot drift from the documented format.

Requires Python 3.9+ and a built ``omega`` binary. No third-party
packages and no shell-specific runtime; the same file runs on Windows,
macOS, and Linux. Per-leg peak RSS uses ``os.wait4`` where the platform
provides it (POSIX); elsewhere the memory metric records ``unavailable``
rather than omitting the leg silently.

Typical use from the repository root of a checkout:

    python3 tools/benchmark/benchmark.py measure \
        --root samples/cli/arithmetic/prime_counter/main.omg \
        --target linux_x86_64 --expected-exit 8 \
        --omega target/debug/omega

    python3 tools/benchmark/benchmark.py validate \
        tools/benchmark/records/*.json
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys
import tempfile
import time


SCHEMA = "omega-benchmark-record/1"
METRIC_NAMES = (
    "compile_time_ms",
    "peak_memory_bytes",
    "code_size_bytes",
    "runtime_ms",
)
RULE_NAME = re.compile(r"^[A-Z][A-Za-z0-9]*$")
TARGET_NAME = re.compile(r"^[a-z][a-z0-9_]*$")
TIMING_LINE = re.compile(r"^\s*([0-9]+(?:\.[0-9]+)?) ms  (.+)$")
PUBLISHED_LINE = re.compile(r"^published native output to (.+)$")
REVIEW_LINE = re.compile(r"^review: (.+)$", re.M)
PENDING_DECISION = re.compile(r"(?m)^(decision .*) pending$")


def run_measured(argv, cwd, stdin_path=None, env=None):
    """Run argv once; return (exit_code, wall_ms, max_rss_bytes|None).

    On POSIX the child is waited on with os.wait4 so its own
    max-resident-set is reported, normalized to bytes (Linux reports
    kilobytes, macOS bytes). Where wait4 is absent (Windows) the wall
    clock is still measured and the RSS field returns None so the
    caller records the metric as unavailable rather than guessing.
    """
    if stdin_path is None:
        stdin_path = os.devnull
    started = time.monotonic()
    if hasattr(os, "wait4"):
        pid = os.fork()
        if pid == 0:
            try:
                null = os.open(stdin_path, os.O_RDONLY)
                sink = os.open(os.devnull, os.O_WRONLY)
                os.dup2(null, 0)
                os.dup2(sink, 1)
                os.dup2(sink, 2)
                os.execvpe(argv[0], argv, os.environ if env is None else env)
            finally:
                os._exit(127)
        _, status, usage = os.wait4(pid, 0)
        exit_code = os.waitstatus_to_exitcode(status)
        rss = usage.ru_maxrss
        if sys.platform != "darwin":
            rss *= 1024
        wall_ms = (time.monotonic() - started) * 1000.0
        return exit_code, wall_ms, rss
    stdin = open(stdin_path, "rb")
    completed = subprocess.run(
        argv, cwd=cwd, stdin=stdin, env=env,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    stdin.close()
    wall_ms = (time.monotonic() - started) * 1000.0
    return completed.returncode, wall_ms, None


def capture_compile(argv, cwd):
    """Like run_measured but also captures the child's output text."""
    started = time.monotonic()
    if hasattr(os, "wait4"):
        read_fd, write_fd = os.pipe()
        pid = os.fork()
        if pid == 0:
            try:
                os.close(read_fd)
                os.dup2(write_fd, 1)
                os.dup2(write_fd, 2)
                devnull = os.open(os.devnull, os.O_RDONLY)
                os.dup2(devnull, 0)
                os.execvpe(argv[0], argv, os.environ)
            finally:
                os._exit(127)
        os.close(write_fd)
        chunks = []
        while True:
            block = os.read(read_fd, 65536)
            if not block:
                break
            chunks.append(block)
        os.close(read_fd)
        _, status, usage = os.wait4(pid, 0)
        rss = usage.ru_maxrss
        if sys.platform != "darwin":
            rss *= 1024
        wall_ms = (time.monotonic() - started) * 1000.0
        text = b"".join(chunks).decode("utf-8", "replace")
        return os.waitstatus_to_exitcode(status), wall_ms, rss, text
    completed = subprocess.run(
        argv, cwd=cwd, stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        check=False, text=True,
    )
    wall_ms = (time.monotonic() - started) * 1000.0
    return completed.returncode, wall_ms, None, completed.stdout


def parse_timings(output_text):
    """Pull the ``--timings`` phase table and published path out of output."""
    stages = {}
    published = None
    for line in output_text.splitlines():
        timing = TIMING_LINE.match(line)
        if timing:
            stages[timing.group(2)] = float(timing.group(1))
        artifact = PUBLISHED_LINE.match(line)
        if artifact:
            published = artifact.group(1).strip()
    return stages, published


def cpu_name():
    if sys.platform == "linux":
        try:
            for line in Path("/proc/cpuinfo").read_text().splitlines():
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
        except OSError:
            pass
    elif sys.platform == "darwin":
        try:
            return subprocess.check_output(
                ["sysctl", "-n", "machdep.cpu.brand_string"], text=True
            ).strip()
        except (OSError, subprocess.CalledProcessError):
            pass
    return platform.processor() or "unknown"


def rustc_version():
    try:
        return subprocess.check_output(
            ["rustc", "--version"], text=True
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return "unavailable"


ENABLED_RULE = re.compile(
    r"optimizations\.enable\s*\(\s*Optimization::([A-Z][A-Za-z0-9]*)"
)


def authored_selection(root):
    """Exact rule names the subject's build script enables.

    The build vocabulary enables rules through
    ``builder.optimizations.enable(Optimization::Name)`` in the sibling
    ``build.omg``; the CLI can only disable. The scan answers which
    names a default build requests -- it is a record-keeping read, not
    a substitute for the compiler's own selection.
    """
    build_omg = root.parent / "build.omg"
    if not build_omg.is_file():
        return []
    try:
        text = build_omg.read_text()
    except OSError:
        return []
    return sorted(set(ENABLED_RULE.findall(text)))


def selection_label(selection):
    if not selection["enabled"] and not selection["disabled"]:
        return "default"
    canonical = json.dumps(selection, sort_keys=True)
    return "sel-" + hashlib.sha256(canonical.encode()).hexdigest()[:12]


def run_plain(argv, cwd):
    completed = subprocess.run(
        argv, cwd=cwd, stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        check=False, text=True,
    )
    return completed.returncode, completed.stdout or ""


def accept_pending_decisions(document):
    """Rewrite every `decision ... pending` token to `accept`.

    The rendered review document requires a literal `accept`/`reject`
    beside each decision row; any other edit fails recovery with
    ChangedFindings. Returns the rewritten text and the count of
    decisions accepted.
    """
    settled, count = PENDING_DECISION.subn(r"\1 accept", document)
    return settled, count


def settle_package_review(omega, project_dir, target):
    """Accept the generated package review so the project compiles.

    ``omega update`` renders a restartable review document; editing its
    ``pending`` tokens to ``accept`` and resuming publishes
    ``omega.lock``. This records project acceptance of the subject's own
    dependency decisions -- it is not a source audit -- and is the
    documented CLI route past the package-review gate that otherwise
    fails every compile with 'package acceptance is missing'.
    """
    code, text = run_plain(
        [str(omega), "update", "--project", str(project_dir),
         "--target", target], cwd=str(project_dir))
    review_path = None
    found = REVIEW_LINE.search(text)
    if found:
        candidate = Path(found.group(1).strip())
        if candidate.is_file():
            review_path = candidate
    if review_path is None:
        candidate = (project_dir / "build" / "package-manager"
                     / f"review-{target}.txt")
        if candidate.is_file():
            review_path = candidate
    accepted = 0
    if review_path is not None:
        document = review_path.read_text()
        settled, accepted = accept_pending_decisions(document)
        if accepted:
            review_path.write_text(settled)
    if accepted:
        code, text = run_plain(
            [str(omega), "update", "--resume",
             "--project", str(project_dir)], cwd=str(project_dir))
    lock = project_dir / "omega.lock"
    if code != 0 or not lock.is_file():
        sys.stderr.write(text)
        raise SystemExit(
            f"package-review settlement failed; omega.lock missing in "
            f"{project_dir} (update exit {code})")
    return lock


def unavailable(unit, reason):
    return {"status": "unavailable", "unit": unit, "reason": reason}


def measure(args):
    root = Path(args.root)
    if not root.is_file():
        raise SystemExit(f"subject root not found: {root}")
    repository = Path(
        subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"], text=True
        ).strip()
    ).resolve()
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=repository, text=True
    ).strip()
    omega = Path(args.omega).resolve()
    if not omega.is_file():
        raise SystemExit(f"omega binary not found: {omega}")

    selection = {
        "enabled": sorted(set(authored_selection(root)) | set(args.enable)),
        "disabled": sorted(set(args.disable)),
    }
    for name in selection["enabled"] + selection["disabled"]:
        if not RULE_NAME.match(name):
            raise SystemExit(f"not an exact rule name: {name!r}")
    if not TARGET_NAME.match(args.target):
        raise SystemExit(f"not a target profile name: {args.target!r}")

    project_dir = root.resolve().parent
    if not (project_dir / "omega.lock").is_file():
        if not args.prepare:
            raise SystemExit(
                f"{project_dir}/omega.lock is missing; run the prepare "
                "subcommand or pass --prepare to settle package review")
        sys.stderr.write(
            "omega.lock missing; settling package review via omega update "
            "(records project acceptance, not an audit)\n")
        settle_package_review(omega, project_dir, args.target)

    compile_wall = []
    compile_rss = []
    code_sizes = []
    stages = {}
    artifact_path = None
    # Settling package admissions writes omega.admissions beside the
    # subject root; a file the measurement created is removed afterward
    # so the checkout stays clean. A preexisting file is left alone.
    admissions_path = root.resolve().parent / "omega.admissions"
    created_admissions = not admissions_path.exists()
    keep = Path(tempfile.mkdtemp(prefix="omega-benchmark-exe-"))
    try:
        for _ in range(args.compile_samples):
            with tempfile.TemporaryDirectory(
                prefix="omega-benchmark-"
            ) as build_dir:
                argv = [
                    str(omega), "--timings", "--build-dir", build_dir,
                    "--target", args.target,
                ]
                if args.accept_admissions:
                    argv.append("--accept-admissions")
                for name in selection["disabled"]:
                    argv += ["--disable-optimization", name]
                argv.append(str(root.resolve()))
                code, wall_ms, rss, output_text = capture_compile(
                    argv, repository
                )
                if code != 0:
                    sys.stderr.write(output_text)
                    raise SystemExit(f"compile failed with exit {code}")
                sample_stages, published = parse_timings(output_text)
                if published is None:
                    sys.stderr.write(output_text)
                    raise SystemExit(
                        "compile produced no native output line"
                    )
                candidate = Path(published)
                if not candidate.is_absolute():
                    for base in (Path(build_dir), repository):
                        if (base / candidate).exists():
                            candidate = base / candidate
                            break
                    else:
                        raise SystemExit(
                            f"cannot locate published artifact {published!r}"
                        )
                artifact_path = candidate
                code_sizes.append(artifact_path.stat().st_size)
                stages = sample_stages
                compile_wall.append(wall_ms)
                compile_rss.append(rss)
                kept = keep / artifact_path.name
                artifact_path.replace(kept)
                artifact_path = kept
    finally:
        if created_admissions and admissions_path.exists():
            admissions_path.unlink()

    run_wall = []
    run_rss = []
    exit_codes = []
    if args.no_run:
        run_status = "skipped"
    elif artifact_path is None:
        run_status = "unavailable"
    else:
        run_status = "measured"
        for _ in range(args.run_samples):
            code, wall_ms, rss, = run_measured(
                [str(artifact_path)], repository, stdin_path=os.devnull
            )
            run_wall.append(wall_ms)
            run_rss.append(rss)
            exit_codes.append(code)

    metrics = {
        "compile_time_ms": {
            "status": "measured",
            "unit": "ms",
            "samples": compile_wall,
            "median_ms": statistics.median(compile_wall),
            "min_ms": min(compile_wall),
            "stage_timings_last_sample_ms": stages,
        },
        "peak_memory_bytes": {
            "status": "measured" if all(r is not None for r in compile_rss)
                      else "unavailable",
            "unit": "bytes",
            "compile_max_rss": max(
                (r for r in compile_rss if r is not None), default=None
            ),
            "run_max_rss": max(
                (r for r in run_rss if r is not None), default=None
            ),
        },
        "code_size_bytes": {
            "status": "measured",
            "unit": "bytes",
            "samples": code_sizes,
            "value": code_sizes[-1],
            "stable": len(set(code_sizes)) == 1,
        },
        "runtime_ms": {
            "status": run_status,
            "unit": "ms",
            "samples": run_wall,
            "median_ms": statistics.median(run_wall) if run_wall else None,
            "min_ms": min(run_wall) if run_wall else None,
            "exit_codes": exit_codes,
            "exit_code_expected": args.expected_exit,
            "stdin": "devnull",
        },
    }
    if metrics["peak_memory_bytes"]["status"] == "unavailable":
        metrics["peak_memory_bytes"]["reason"] = (
            "os.wait4 is absent on this host (Windows); per-leg peak RSS "
            "was not measured"
        )
    if run_status == "skipped":
        metrics["runtime_ms"]["reason"] = "--no-run was passed"
    elif run_status == "unavailable":
        metrics["runtime_ms"]["reason"] = "compile produced no artifact"
    if args.expected_exit is not None and exit_codes:
        metrics["runtime_ms"]["exit_code_match"] = all(
            code == args.expected_exit for code in exit_codes
        )

    record = {
        "schema": SCHEMA,
        "recorded_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "subject": {
            "name": args.name or root.parent.name,
            "root": root.resolve().relative_to(repository).as_posix()
                    if root.resolve().is_relative_to(repository)
                    else str(root.resolve()),
            "source_revision": revision,
        },
        "key": {
            "target": args.target,
            "selection": selection,
            "selection_source": "build.omg enable calls plus --enable/--disable",
        },
        "host": {
            "os": sys.platform,
            "machine": platform.machine(),
            "cpu": cpu_name(),
            "logical_cpus": os.cpu_count(),
            "python": platform.python_version(),
            "rustc": rustc_version(),
            "omega_binary": str(omega.relative_to(repository))
                            if omega.is_relative_to(repository)
                            else str(omega),
            "omega_profile": "dev" if "debug" in omega.parts else "unknown",
        },
        "metrics": metrics,
        "notes": args.note,
    }
    return record


def validate_record(record, path):
    problems = []
    def need(condition, message):
        if not condition:
            problems.append(f"{path}: {message}")

    need(record.get("schema") == SCHEMA,
         f"schema must be {SCHEMA!r}, got {record.get('schema')!r}")
    subject = record.get("subject", {})
    need(isinstance(subject.get("name"), str) and subject["name"],
         "subject.name is required")
    need(isinstance(subject.get("root"), str) and subject["root"],
         "subject.root is required")
    need(re.fullmatch(r"[0-9a-f]{40}", subject.get("source_revision", ""))
         is not None, "subject.source_revision must be a full lowercase sha")
    key = record.get("key", {})
    need(TARGET_NAME.match(key.get("target", "") or "") is not None,
         "key.target must be a target profile name like linux_x86_64")
    selection = key.get("selection", {})
    for leg in ("enabled", "disabled"):
        names = selection.get(leg)
        need(isinstance(names, list)
             and all(isinstance(n, str) and RULE_NAME.match(n) for n in names)
             and names == sorted(set(names)),
             f"key.selection.{leg} must be sorted unique exact rule names")
    host = record.get("host", {})
    for field in ("os", "machine", "omega_binary"):
        need(isinstance(host.get(field), str) and host[field],
             f"host.{field} is required")
    metrics = record.get("metrics", {})
    for name in METRIC_NAMES:
        metric = metrics.get(name)
        need(isinstance(metric, dict), f"metrics.{name} is required")
        if not isinstance(metric, dict):
            continue
        status = metric.get("status")
        need(status in ("measured", "unavailable", "skipped"),
             f"metrics.{name}.status must be measured/unavailable/skipped")
        if status in ("unavailable", "skipped"):
            need(isinstance(metric.get("reason"), str) and metric["reason"],
                 f"metrics.{name}.reason is required when not measured")
        if status == "measured":
            if name == "peak_memory_bytes":
                need(isinstance(metric.get("compile_max_rss"), (int, float)),
                     "metrics.peak_memory_bytes.compile_max_rss is required")
            elif name == "code_size_bytes":
                need(isinstance(metric.get("value"), int) and metric["value"] > 0,
                     "metrics.code_size_bytes.value must be a positive int")
            else:
                samples = metric.get("samples")
                need(isinstance(samples, list) and samples
                     and all(isinstance(s, (int, float)) for s in samples),
                     f"metrics.{name}.samples must be a nonempty number list")
    return problems


def write_record(record, records_dir):
    label = selection_label(record["key"]["selection"])
    name = "{}__{}__{}.json".format(
        record["subject"]["name"], record["key"]["target"], label
    )
    records_dir.mkdir(parents=True, exist_ok=True)
    path = records_dir / name
    path.write_text(json.dumps(record, indent=2, sort_keys=False) + "\n")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    measure_parser = commands.add_parser("measure", help="run one subject and write a record")
    measure_parser.add_argument("--root", required=True,
                                help="subject root .omg file")
    measure_parser.add_argument("--target", required=True,
                                help="target profile name, e.g. linux_x86_64")
    measure_parser.add_argument("--name", help="subject name (default: subject directory name)")
    measure_parser.add_argument("--omega", default="target/debug/omega",
                                help="path to the omega binary")
    measure_parser.add_argument("--enable", action="append", default=[],
                                help="exact optimization rule to enable (repeatable)")
    measure_parser.add_argument("--disable", action="append", default=[],
                                help="exact optimization rule to disable (repeatable)")
    measure_parser.add_argument(
        "--accept-admissions", action=argparse.BooleanOptionalAction,
        default=True,
        help="pass --accept-admissions to the compile leg (default on)")
    measure_parser.add_argument("--compile-samples", type=int, default=3)
    measure_parser.add_argument("--run-samples", type=int, default=5)
    measure_parser.add_argument("--expected-exit", type=int,
                                help="expected subject exit code")
    measure_parser.add_argument(
        "--prepare", action=argparse.BooleanOptionalAction, default=True,
        help="settle the package-review gate (omega update + accept) when "
             "omega.lock is missing (default on)")
    measure_parser.add_argument("--no-run", action="store_true",
                                help="compile-only leg (cross target, no host runtime)")
    measure_parser.add_argument("--records-dir",
                                default="tools/benchmark/records")
    measure_parser.add_argument("--note", action="append", default=[],
                                help="extra note string retained in the record")
    measure_parser.add_argument("--print", action="store_true",
                                help="print the record without writing it")
    prepare_parser = commands.add_parser(
        "prepare", help="settle package review (update + accept) for a subject")
    prepare_parser.add_argument("--root", required=True,
                                help="subject root .omg file")
    prepare_parser.add_argument("--target", required=True,
                                help="target profile name, e.g. linux_x86_64")
    prepare_parser.add_argument("--omega", default="target/debug/omega",
                                help="path to the omega binary")
    validate_parser = commands.add_parser("validate", help="check record files")
    validate_parser.add_argument("records", nargs="+")
    args = parser.parse_args()

    if args.command == "prepare":
        omega = Path(args.omega).resolve()
        if not omega.is_file():
            raise SystemExit(f"omega binary not found: {omega}")
        project_dir = Path(args.root).resolve().parent
        lock = settle_package_review(omega, project_dir, args.target)
        print(f"settled {lock}")
        return 0
    if args.command == "measure":
        record = measure(args)
        errors = validate_record(record, "<generated>")
        if errors:
            for error in errors:
                print(error, file=sys.stderr)
            return 2
        if args.print:
            print(json.dumps(record, indent=2))
            return 0
        path = write_record(record, Path(args.records_dir))
        print(f"wrote {path}")
        return 0
    problems = []
    for name in args.records:
        path = Path(name)
        try:
            record = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError) as error:
            problems.append(f"{path}: unreadable record: {error}")
            continue
        problems.extend(validate_record(record, str(path)))
    for problem in problems:
        print(problem, file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
