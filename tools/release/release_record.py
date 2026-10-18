#!/usr/bin/env python3
"""Release-record substrate for the Rust compiler completion matrix.

`wiki/drafts/reference/rust_compiler_completion.md` defines six named gates and four
required platform runs, and requires the release record to contain the commit,
pinned Rust toolchain, host OS and architecture, commands, results, elapsed
time, and the exact list of expected skips. This tool is that record's only
recorder: it owns the gate manifest verbatim, runs a gate's command block on
one host, writes one versioned JSON record per runner row under `records/`
so a row's evidence survives the machine that produced it, and joins the four
lanes' records into the release closure.

The harness records outcomes; it does not judge the matrix. A red gate is
recorded as `fail` and the closure stays `open` — the substrate never turns a
failed or missing row into a pass. A gate whose commands exit 0 still reads
`open` unless their output proves a complete run: every skipped test is named
and declared as an expected skip, nothing was filtered out, a
test/fixture/sample command executed at least one item, the checkout was
clean, and no inherited environment variable narrowed the selection. A
command that timed out, could not start, or that this host cannot run
(missing cargo-nextest, or beyond `--limit`) is recorded with its reason and
leaves its gate `open`. `check` re-derives every gate status and the closure
from that evidence, so a hand-edited or truncated record cannot read as
closed.

One record evidences one lane, so its own closure always lists the other
three lanes. `close` is the only route to a closed release: it validates one
record per lane, rejects mixed commits or toolchains and lanes recorded by
another lane's record, and emits one closure document listing the exact open
rows per host.

Requirements: Python 3.9+ and Git. Standard library only; identical under
PowerShell on Windows and sh on macOS/Linux. Commands run through `mbx` when
it is installed and through Cargo otherwise, per AGENTS.md.

    python3 tools/release/release_record.py plan
    python3 tools/release/release_record.py run --target linux_x86_64 \
        --native-execution "python3 tools/corpus_gate.py --native"
    python3 tools/release/release_record.py run --target linux_arm64 \
        --emulator "qemu-aarch64 9.0.0" --gate RC-NATIVE-MATRIX \
        --native-execution "qemu-aarch64 <emitted-elf>"
    python3 tools/release/release_record.py check records/<file>.json
    python3 tools/release/release_record.py close \
        records/*__<commit[:10]>__*.json --commit <commit> --output closure.json

A runner row is only `recorded` when the record carries a passing
`--native-execution` observation — direct execution of the emitted programs —
and `run` refuses a lane this host cannot execute (the `linux_arm64` lane
also accepts any host under a named emulator).
"""

import argparse
import functools
import hashlib
import json
import os
import platform
import re
import shlex
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

# Schema 2 added the run-level evidence (`environment_filters`,
# `worktree_clean`) that a gate needs before it can read `pass`. Schema 3
# lets a command row carry `exit: null` with an `incomplete` reason (timeout,
# unavailable tool, `--limit`) and a `log` path, and derives `unlisted_skips`
# per gate from the declared expected skips.
SCHEMA = "omega-release-record/3"
CLOSURE_SCHEMA = "omega-release-closure/1"

ROOT = Path(__file__).resolve().parents[2]
TOOLCHAIN_FILE = ROOT / "rust-toolchain.toml"
RECORDS_DIR = Path(__file__).resolve().parent / "records"
# Full command output is host-local diagnosis, not committed evidence: the
# record keeps a bounded tail and names the log file under the ignored target.
LOGS_DIR = ROOT / "target" / "release_record"

# The six gates of the release matrix, verbatim from
# wiki/drafts/reference/rust_compiler_completion.md. Each gate runs its command block
# through the resolved `mbx`/`cargo` wrapper. A test in
# tools/tests/test_release_record.py pins this manifest against the document
# so the substrate cannot silently diverge from the contract it records.
GATES = {
    "RC-REPOSITORY": {
        "capability": "The pinned toolchain formats, lints, type-checks, and "
                      "preserves architectural dependency boundaries.",
        "commands": [
            "python tools/fmt.py --check",
            "mbx clippy --workspace --all-targets -- -D warnings",
            "mbx check --workspace --all-targets",
            "mbx nextest run --workspace --lib --no-fail-fast",
        ],
    },
    "RC-SOURCE-SEMANTICS": {
        "capability": "Every accepted positive fixture reaches its promised "
                      "checked or product stage; every negative fixture "
                      "rejects; individual semantic integration tests pass.",
        "commands": [
            "python tools/corpus_gate.py",
        ],
    },
    "RC-BUILD-AND-PACKAGES": {
        "capability": "Build declarations, immutable inputs, package "
                      "identities, reviewed evidence, resolution, and "
                      "compilation handoff agree without path/name inference "
                      "or hidden ambient mutation.",
        "commands": [
            "mbx nextest run -p build-declarations -p package-source "
            "-p resolver-execution --no-fail-fast",
            "mbx test --doc -p build-declarations -p package-source "
            "-p resolver-execution",
        ],
    },
    "RC-NATIVE-MATRIX": {
        "capability": "Each hosted target produces independently validated "
                      "machine code, object/image bytes, ABI behavior, "
                      "provider settlement, and observable execution on its "
                      "matching host.",
        "commands": [
            "python tools/corpus_gate.py --native",
        ],
        "per_host": True,
    },
    "RC-DIAGNOSTICS": {
        "capability": "Rejected source and failed product admission report "
                      "stable, actionable diagnostics rather than panics, "
                      "silent fallback, or accidental acceptance.",
        "commands": [
            "python tools/corpus_gate.py --filter fail/",
        ],
    },
    "RC-REPRESENTATIVE-PROGRAMS": {
        "capability": "Every maintained sample reaches checked semantics; "
                      "every sample with an authored host entry reaches its "
                      "native product; every documented deterministic "
                      "exit/output oracle passes.",
        "commands": [
            "python tools/sample_native_gate.py",
        ],
        "per_host": True,
    },
}

# Required platform runs from the completion contract's runner table. Each
# lane's `hosts` enumerates the (platform.system(), platform.machine()) pairs,
# lowercased, that may record the row — the contract requires direct execution
# of the emitted programs on the matching host. The `linux_arm64` lane alone
# also accepts any host when the record names the emulator used.
PLATFORM_RUNNERS = (
    {"runner": "Linux x86-64", "target": "linux_x86_64",
     "execution": "direct",
     "hosts": (("linux", "x86_64"), ("linux", "amd64"))},
    {"runner": "Linux AArch64", "target": "linux_arm64",
     "execution": "direct-or-named-emulator",
     "hosts": (("linux", "aarch64"), ("linux", "arm64"))},
    {"runner": "macOS AArch64", "target": "macos_arm64",
     "execution": "direct",
     "hosts": (("darwin", "arm64"), ("darwin", "aarch64"))},
    {"runner": "Windows x86-64", "target": "windows_x86_64",
     "execution": "direct",
     "hosts": (("windows", "amd64"), ("windows", "x86_64"))},
)

# `SKIP [         ] (───) <binary-id> <test-name>` at nextest's `skip` status
# level; the counter column is optional. A name is `<binary-id> <test-name>`.
NEXTEST_SKIPPED = re.compile(
    r"^\s*SKIP\s+\[[^\]]*\]\s+(?:\([^)]*\)\s+)?(\S+)\s+(\S+)\s*$", re.M)
NEXTEST_SUMMARY = re.compile(r"(\d+) skipped")
NEXTEST_RUN = re.compile(r"(\d+) tests? run:")
CARGO_TEST_IGNORED = re.compile(r"^test (.+) \.\.\. ignored$", re.M)
CARGO_TEST_RESULT = re.compile(
    r"test result: \w+\. \d+ passed; \d+ failed; (\d+) ignored")
CARGO_TEST_FILTERED = re.compile(r"(\d+) filtered out")
CORPUS_FIXTURES = re.compile(r"(\d+) fixtures \(")
SAMPLES_EXECUTED = re.compile(r"sample_native_gate: \d+/(\d+) samples meet")
# Inherited environment variables that narrow what a gate command selects.
# A run made under any of them evidences a subset, so its gates stay open.
FILTER_ENVIRONMENT = (
    "OMEGA_CORPUS_FIXTURE_FILTER",
    "OMEGA_CORPUS_SHARD",
    "NEXTEST_PROFILE",
    "NEXTEST_RUN_IGNORED",
)
# nextest names skipped tests only at the `skip` status level; without the
# names a skip count cannot be reconciled with the expected-skip list.
NAMED_SKIP_ENVIRONMENT = {"NEXTEST_STATUS_LEVEL": "skip"}
COMMIT = re.compile(r"^[0-9a-f]{40}$")
TARGET_NAME = re.compile(r"^[a-z][a-z0-9_]*$")
# Captured output is evidence, not a log store: keep the tail bounded.
OUTPUT_TAIL_BYTES = 64 * 1024


# `Doc-tests <crate>` precedes each crate's doc-test harness output; ignored
# doc tests are qualified by it because two crates can share a doc-test name.
DOC_TESTS_HEADER = re.compile(r"^\s*Doc-tests (\S+)\s*$")


def host_lanes(os_name=None, machine=None):
    """Lanes whose matching host is the given (or current) host."""
    os_name = (os_name if os_name is not None else platform.system()).lower()
    machine = (machine if machine is not None
               else platform.machine()).lower()
    return [row["target"] for row in PLATFORM_RUNNERS
            if (os_name, machine) in row["hosts"]]


def lane_host_match(target, emulator=None, os_name=None, machine=None):
    """Whether the given (or current) host may record the lane's row.

    Direct lanes require the contract's matching host. The `linux_arm64` row
    additionally accepts a named emulator from any host, per the contract's
    "emulation is acceptable only when the release record names the emulator
    and version" allowance.
    """
    return target in host_lanes(os_name, machine) or (
        target == "linux_arm64" and bool(emulator))


def available_runner():
    """The `mbx` wrapper when installed, otherwise Cargo, otherwise None."""
    return shutil.which("mbx") or shutil.which("cargo")


def resolve_command(text, selected_runner):
    argv = shlex.split(text)
    if argv and argv[0] == "mbx":
        argv[0] = selected_runner
    elif argv and argv[0] == "python":
        # The contract's portable Python legs (`tools/fmt.py`, the corpus
        # gate) run on this interpreter, never through the Cargo wrapper.
        argv[0] = sys.executable
    return argv


@functools.lru_cache(maxsize=None)
def nextest_available(selected_runner):
    try:
        completed = subprocess.run(
            [selected_runner, "nextest", "--version"], cwd=ROOT,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
    except OSError:
        return False
    return completed.returncode == 0


def command_unavailable(text, selected_runner):
    """Why this host cannot run a contract command, or None when it can.

    A missing tool is the contract's "unavailable runtime dependency": the
    command is recorded as not run and its gate stays open, rather than
    running and reading as a test failure.
    """
    argv = shlex.split(text)
    if argv and argv[0] == "python":
        return None
    if argv and argv[0] == "mbx" and selected_runner is None:
        return "mbx and Cargo unavailable"
    if "nextest" in argv and not nextest_available(selected_runner):
        return "cargo-nextest unavailable"
    return None


def toolchain_channel():
    channel = None
    for line in TOOLCHAIN_FILE.read_text(encoding="utf-8").splitlines():
        match = re.match(r'^\s*channel\s*=\s*"([^"]+)"', line)
        if match:
            channel = match.group(1)
    if channel is None:
        raise ValueError("rust-toolchain.toml has no channel entry")
    return channel


def rustc_version():
    # rustup proxies `rustc` to the channel pinned by rust-toolchain.toml.
    completed = subprocess.run(
        ["rustc", "--version"], cwd=ROOT, capture_output=True,
        text=True, check=True)
    return completed.stdout.strip()


def commit_sha():
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True,
        text=True, check=True)
    return completed.stdout.strip()


def worktree_clean(excluded=()):
    """Whether the checkout has no changes outside the excluded directories.

    The records directory is excluded: an earlier run's record there cannot
    change what a gate compiles, and counting it would make every later run
    in the same checkout read dirty.
    """
    pathspecs = ["."]
    for directory in excluded:
        try:
            relative = Path(directory).resolve().relative_to(ROOT)
        except ValueError:
            continue
        pathspecs.append(":(exclude){}".format(relative.as_posix()))
    completed = subprocess.run(
        ["git", "status", "--porcelain", "--", *pathspecs], cwd=ROOT,
        capture_output=True, text=True, check=True)
    return not completed.stdout.strip()


def environment_filters(environment):
    return {name: environment[name] for name in FILTER_ENVIRONMENT
            if environment.get(name, "").strip()}


def skipped_test_names(output):
    """Every skipped test a nextest or doc-test run names in its output."""
    names = {"{} {}".format(binary, test)
             for binary, test in NEXTEST_SKIPPED.findall(output)}
    harness = None
    for line in output.splitlines():
        header = DOC_TESTS_HEADER.match(line)
        if header:
            harness = header.group(1)
            continue
        ignored = CARGO_TEST_IGNORED.match(line)
        if ignored:
            names.add(ignored.group(1) if harness is None
                      else "{} {}".format(harness, ignored.group(1)))
    return sorted(names)


def skipped_test_count(output):
    return (sum(int(value) for value in NEXTEST_SUMMARY.findall(output))
            + sum(int(value) for value in CARGO_TEST_RESULT.findall(output)))


def output_text(value):
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value


def display_path(target):
    try:
        return Path(target).resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return str(target)


def command_row(command, argv, exit_code, elapsed_ms, output, incomplete,
                log):
    # The count comes from the runners' own summaries and the names from
    # their per-test lines; a record reconciles the two rather than
    # substituting one for the other.
    return {
        "command": command,
        "argv": argv,
        "exit": exit_code,
        "incomplete": incomplete,
        "elapsed_ms": elapsed_ms,
        "skipped_tests": skipped_test_count(output),
        "skipped_names": skipped_test_names(output),
        "output_tail": output[-OUTPUT_TAIL_BYTES:],
        "log": log,
    }


def kill_process_tree(process):
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            stdin=subprocess.DEVNULL,
            capture_output=True,
        )
    else:
        import signal
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
    try:
        process.kill()
    except OSError:
        pass


def run_command(command, selected_runner, timeout=None, log_path=None):
    """Run one command and keep its evidence; never raise on its failure.

    stdout and stderr share one pipe so the stored tail keeps the runners'
    chronological order (a doc-test header before its results, the final
    summary last). A timeout or a command that cannot start is recorded with
    `exit: null` and its reason, which leaves the gate open.
    """
    argv = resolve_command(command, selected_runner)
    started = time.monotonic()
    exit_code = None
    incomplete = None
    try:
        with subprocess.Popen(
            argv, cwd=ROOT, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            encoding="utf-8", errors="replace",
            start_new_session=(os.name != "nt"),
            env={**os.environ, **NAMED_SKIP_ENVIRONMENT},
        ) as process:
            try:
                stdout, _ = process.communicate(timeout=timeout)
                exit_code = process.returncode
                output = output_text(stdout)
            except subprocess.TimeoutExpired:
                # The contract: a timeout leaves the row open; it is not a pass.
                kill_process_tree(process)
                stdout, _ = process.communicate()
                output = output_text(stdout)
                incomplete = "timeout after {:g} s".format(timeout)
    except OSError as error:
        output = ""
        incomplete = "could not start: {}".format(error)
    elapsed_ms = (time.monotonic() - started) * 1000.0
    log = None
    if log_path is not None:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        log_path.write_text(output, encoding="utf-8")
        log = display_path(log_path)
    return command_row(command, argv, exit_code, elapsed_ms, output,
                       incomplete, log)


def not_run_command(command, selected_runner, reason):
    argv = resolve_command(command, selected_runner or "cargo")
    return command_row(command, argv, None, 0.0, "", reason, None)


def run_gate_commands(gate_name, commands, selected_runner, timeout=None,
                      limit=None, log_dir=None):
    """Run a gate's command block, recording every command in order.

    Commands beyond `limit` or unavailable on this host are recorded as not
    run with the reason, so the record keeps the contract's command count
    and the gate reads open instead of silently shrinking.
    """
    rows = []
    for index, command in enumerate(commands):
        print("$ {}".format(command), flush=True)
        if limit is not None and index >= limit:
            row = not_run_command(command, selected_runner, "beyond --limit")
        else:
            reason = command_unavailable(command, selected_runner)
            if reason is not None:
                row = not_run_command(command, selected_runner, reason)
            else:
                log_path = (None if log_dir is None else
                            Path(log_dir) / "{}-{}.log".format(gate_name,
                                                               index))
                row = run_command(command, selected_runner, timeout=timeout,
                                  log_path=log_path)
        rows.append(row)
        if row["exit"] is None:
            print("    not complete: {}".format(row["incomplete"]),
                  flush=True)
        else:
            print("    exit {} in {:.0f} ms{}".format(
                row["exit"], row["elapsed_ms"],
                " ({} skipped)".format(row["skipped_tests"])
                if row["skipped_tests"] else ""), flush=True)
    return rows


def executed_units(argv, output):
    """Items a command reports executing, or None when it executes none.

    nextest reports tests run, a doc-test command one result per crate
    harness, the corpus gate its fixtures and sample refresh its samples.
    Format, lint and check commands compile without executing anything.
    """
    arguments = [str(argument) for argument in argv]
    if "nextest" in arguments:
        counts = NEXTEST_RUN.findall(output)
        return int(counts[-1]) if counts else 0
    if "test" in arguments and "--doc" in arguments:
        return len(CARGO_TEST_RESULT.findall(output))
    if any(argument.endswith("corpus_gate.py") for argument in arguments):
        counts = CORPUS_FIXTURES.findall(output)
        return int(counts[-1]) if counts else 0
    if any(argument.endswith("sample_native_gate.py") for argument in arguments):
        counts = SAMPLES_EXECUTED.findall(output)
        return int(counts[-1]) if counts else 0
    return None


def command_evidence_complete(row):
    """Whether an exit-zero command's output proves a complete run.

    Every counted skip must be named, no test may be filtered out, and a
    command that executes tests, fixtures or samples must execute at least
    one. The checks read the stored output tail, so `check` derives the same
    answer from a record that `run` derived from the live output.
    """
    names = row.get("skipped_names")
    if not isinstance(names, list) or row.get("skipped_tests") != len(names):
        return False
    output = row.get("output_tail") or ""
    if any(int(count) for count in CARGO_TEST_FILTERED.findall(output)):
        return False
    executed = executed_units(row.get("argv") or [], output)
    return executed is None or executed > 0


def run_evidence_complete(record):
    """A clean checkout under no filtering environment, recorded as such."""
    return (record.get("worktree_clean") is True
            and record.get("environment_filters") == {})


def exit_code_of(row):
    exit_code = row.get("exit")
    return exit_code if type(exit_code) is int else None


def gate_status(commands, run_complete, expected_skips=frozenset()):
    """`fail` on a nonzero exit; `pass` only on complete evidence.

    A command without an exit (timeout, unavailable tool, `--limit`) or with
    a skipped test the record did not declare as an expected skip of this
    gate leaves the gate `open`: the contract counts an unexpected ignored
    test, a timeout and an unavailable runtime dependency as open, never as
    a pass.
    """
    if any(exit_code_of(row) not in (None, 0) for row in commands):
        return "fail"
    if (commands and run_complete
            and all(exit_code_of(row) == 0
                    and command_evidence_complete(row)
                    and set(row.get("skipped_names") or ()) <= expected_skips
                    for row in commands)):
        return "pass"
    return "open"


def observation_recorded(observation):
    return (isinstance(observation, dict)
            and exit_code_of(observation) == 0
            and command_evidence_complete(observation))


def declared_skips(expected_skips, gate_name):
    return frozenset(
        skip.get("test") for skip in expected_skips or []
        if isinstance(skip, dict) and skip.get("gate") == gate_name)


def unlisted_skips(gates, expected_skips):
    """Skipped test names a gate observed but the record did not declare
    as an expected skip of that same gate."""
    return sorted({
        name for gate in gates
        for row in gate.get("commands") or []
        for name in row.get("skipped_names") or []
        if name not in declared_skips(expected_skips, gate.get("name"))})


def open_rows(gates, platform_runs):
    """Every gate that is not `pass` or not run, then every unrecorded lane."""
    statuses = {gate["name"]: gate["status"] for gate in gates}
    return (
        [name for name in GATES if name in statuses
         and statuses[name] != "pass"]
        + [name for name in GATES if name not in statuses]
        + [row["target"] for row in platform_runs
           if row["status"] != "recorded"])


def parse_expected_skip(entry):
    parts = entry.split("|", 2)
    if len(parts) != 3 or not all(part.strip() for part in parts):
        raise ValueError(
            "--expect-skip takes GATE|TEST|REASON, got {!r}".format(entry))
    gate, test, reason = (part.strip() for part in parts)
    if gate not in GATES:
        raise ValueError("--expect-skip names unknown gate {!r}".format(gate))
    return {"gate": gate, "test": test, "reason": reason}


def build_record(target, emulator, observation, gate_names, results,
                 expected_skips, selected_runner, clean, filters):
    run_complete = clean and not filters
    gates = []
    for name in gate_names:
        gate = GATES[name]
        commands = results[name]
        status = gate_status(commands, run_complete,
                             declared_skips(expected_skips, name))
        gates.append({
            "name": name,
            "capability": gate["capability"],
            "status": status,
            "commands": commands,
        })
    platform_runs = [{
        "runner": row["runner"],
        "target": row["target"],
        # A lane reads recorded only with a passed direct-execution
        # observation; a red or missing probe leaves the row open.
        "status": ("recorded" if row["target"] == target
                   and observation_recorded(observation) else "open"),
        "emulator": (emulator if row["target"] == target else None),
        "observation": (observation if row["target"] == target else None),
    } for row in PLATFORM_RUNNERS]
    rows = open_rows(gates, platform_runs)
    return {
        "schema": SCHEMA,
        "recorded_utc": datetime.now(timezone.utc).strftime(
            "%Y-%m-%dT%H:%M:%SZ"),
        "commit": commit_sha(),
        "toolchain": {
            "channel": toolchain_channel(),
            "rustc": rustc_version(),
            "runner": selected_runner,
        },
        "host": {
            "os": platform.system().lower(),
            "machine": platform.machine(),
            "python": platform.python_version(),
        },
        "target": target,
        "emulator": emulator,
        "worktree_clean": clean,
        "environment_filters": filters,
        "gates": gates,
        "platform_runs": platform_runs,
        "expected_skips": expected_skips,
        "unlisted_skips": unlisted_skips(gates, expected_skips),
        "closure": {
            "status": "open" if rows else "closed",
            "open_rows": rows,
        },
    }


def write_record(record, records_dir):
    records_dir.mkdir(parents=True, exist_ok=True)
    stamp = record["recorded_utc"].replace(":", "").replace("-", "")
    path = records_dir / "{}__{}__{}.json".format(
        record["target"], record["commit"][:10], stamp)
    path.write_text(
        json.dumps(record, indent=2, sort_keys=False) + "\n",
        encoding="utf-8")
    return path


def command_evidence_shaped(row):
    """An exit with no incomplete reason, or no exit with a stated reason."""
    exit_code = row.get("exit")
    incomplete = row.get("incomplete")
    if type(exit_code) is int:
        return incomplete is None
    return (exit_code is None and isinstance(incomplete, str)
            and bool(incomplete.strip()))


def validate_record(record, path):
    problems = []

    def need(condition, message):
        if not condition:
            problems.append(message)

    need(record.get("schema") == SCHEMA,
         "schema must be {!r}".format(SCHEMA))
    need(isinstance(record.get("commit"), str)
         and COMMIT.match(record["commit"]),
         "commit must be a full lowercase sha")
    toolchain = record.get("toolchain") or {}
    need(bool(toolchain.get("channel")), "toolchain.channel is required")
    need(bool(toolchain.get("rustc")), "toolchain.rustc is required")
    host = record.get("host") or {}
    need(bool(host.get("os")) and bool(host.get("machine")),
         "host os and machine are required")
    need(record.get("target") in {row["target"] for row in PLATFORM_RUNNERS},
         "target must name a required platform runner")
    need(isinstance(record.get("worktree_clean"), bool),
         "worktree_clean must record whether the checkout was clean")
    need(isinstance(record.get("environment_filters"), dict),
         "environment_filters must record the inherited filter variables")
    expected_skips = record.get("expected_skips")
    need(isinstance(expected_skips, list)
         and all(isinstance(skip, dict) for skip in expected_skips),
         "expected_skips must be the exact recorded list")
    if not isinstance(expected_skips, list):
        expected_skips = []
    expected_skips = [skip for skip in expected_skips
                      if isinstance(skip, dict)]
    run_complete = run_evidence_complete(record)
    gates = {gate.get("name"): gate for gate in record.get("gates") or []}
    need(set(gates) <= set(GATES),
         "gates {} are not in the contract".format(
             sorted(str(name) for name in set(gates) - set(GATES))))
    for name, gate in GATES.items():
        if name not in gates:
            continue
        row = gates[name]
        commands = row.get("commands") or []
        derived = gate_status(commands, run_complete,
                              declared_skips(expected_skips, name))
        need(row.get("status") == derived,
             "gate {} status {} contradicts its command evidence "
             "(expected {})".format(name, row.get("status"), derived))
        need(len(commands) == len(gate["commands"]),
             "gate {} records {} commands, contract has {}".format(
                 name, len(commands), len(gate["commands"])))
        for observed, expected in zip(commands, gate["commands"]):
            need(observed.get("command") == expected,
                 "gate {} command drifted from the contract".format(name))
            need(command_evidence_shaped(observed)
                 and isinstance(observed.get("elapsed_ms"), (int, float)),
                 "gate {} command lacks exit/elapsed evidence".format(name))
            need(isinstance(observed.get("skipped_tests"), int)
                 and isinstance(observed.get("skipped_names"), list),
                 "gate {} command lacks skip evidence".format(name))
    derived_unlisted = unlisted_skips(list(gates.values()), expected_skips)
    need(record.get("unlisted_skips") == derived_unlisted,
         "unlisted_skips {} contradicts the skipped tests missing from "
         "expected_skips (expected {})".format(
             record.get("unlisted_skips"), derived_unlisted))
    for skip in expected_skips:
        need(isinstance(skip.get("reason"), str) and skip["reason"].strip(),
             "expected skip {} states no reason".format(skip.get("test")))
        gate = gates.get(skip.get("gate"))
        if gate is None:
            problems.append(
                "expected skip {} names gate {}, which this record did not "
                "run".format(skip.get("test"), skip.get("gate")))
            continue
        observed = {name for row in gate.get("commands") or []
                    for name in row.get("skipped_names") or []}
        need(skip.get("test") in observed,
             "expected skip {} was declared but not observed in {}".format(
                 skip.get("test"), skip.get("gate")))
    runs = {row.get("target"): row for row in record.get("platform_runs") or []}
    for row in PLATFORM_RUNNERS:
        lane = runs.get(row["target"])
        need(lane is not None,
             "platform run {} is missing".format(row["target"]))
        if not isinstance(lane, dict) or lane.get("status") != "recorded":
            continue
        # A row may only be recorded by its own lane's record — the sibling
        # rows are evidence produced by each lane's own host.
        need(row["target"] == record.get("target"),
             "platform run {} cannot be recorded by another lane's record"
             .format(row["target"]))
        observation = lane.get("observation")
        need(observation_recorded(observation)
             and isinstance(observation.get("command"), str)
             and observation["command"].strip()
             and isinstance(observation.get("argv"), list)
             and isinstance(observation.get("elapsed_ms"), (int, float)),
             "platform run {} is recorded without a passing, complete "
             "direct-execution observation".format(row["target"]))
        need(lane_host_match(
                row["target"], emulator=record.get("emulator"),
                os_name=host.get("os"), machine=host.get("machine")),
             "recorded host {}/{} cannot execute lane {}"
             .format(host.get("os"), host.get("machine"), row["target"]))
    recorded = runs.get(record.get("target")) or {}
    if recorded.get("emulator") is not None:
        need(record["target"] == "linux_arm64",
             "emulator is permitted only on the linux_arm64 row")
    closure = record.get("closure") or {}
    derived_gates = [
        {"name": name,
         "status": gate_status(gates[name].get("commands") or [],
                               run_complete,
                               declared_skips(expected_skips, name))}
        for name in GATES if name in gates]
    derived_runs = [
        {"target": row["target"],
         "status": (runs.get(row["target"]) or {}).get("status")}
        for row in PLATFORM_RUNNERS]
    expected_rows = open_rows(derived_gates, derived_runs)
    need(closure.get("open_rows") == expected_rows,
         "closure.open_rows {} contradicts the recorded rows (expected {})"
         .format(closure.get("open_rows"), expected_rows))
    expected_closure = "open" if expected_rows else "closed"
    need(closure.get("status") == expected_closure,
         "closure.status {} contradicts the recorded rows (expected {})"
         .format(closure.get("status"), expected_closure))
    if problems:
        raise ValueError("{}: {}".format(path, "; ".join(problems)))


def lane_gate_statuses(record):
    """Each contract gate's status in one validated lane record."""
    statuses = {gate["name"]: gate["status"]
                for gate in record.get("gates") or []}
    return {name: statuses.get(name, "not run") for name in GATES}


def own_platform_run(record):
    """The record's row for its own lane (validation guarantees one)."""
    return next(row for row in record["platform_runs"]
                if isinstance(row, dict)
                and row.get("target") == record["target"])


def close_records(entries, commit=None):
    """Join one validated record per lane into the release closure.

    `entries` holds `(path, record, sha256)` for each record file. Rejects
    (ValueError) an invalid record, a missing or duplicated lane, a record
    whose host cannot execute its own lane, records at different commits or
    at any commit other than `commit` when one is named, and records made
    under different toolchains. A valid join returns the closure document:
    `closed` only when every gate passes in every record that ran it, each
    per-host gate passes on every lane, every other gate ran on some lane,
    and every lane's own record carries its direct-execution observation.
    """
    problems = []
    for path, record, _ in entries:
        try:
            validate_record(record, path)
        except ValueError as error:
            problems.append(str(error))
    if problems:
        raise ValueError("; ".join(problems))
    lanes = [row["target"] for row in PLATFORM_RUNNERS]
    by_lane = {lane: [] for lane in lanes}
    for entry in entries:
        by_lane[entry[1]["target"]].append(entry)
    for lane in lanes:
        if not by_lane[lane]:
            problems.append("lane {} has no record".format(lane))
        elif len(by_lane[lane]) > 1:
            problems.append("lane {} has {} records ({}); join exactly one "
                            "per lane".format(
                                lane, len(by_lane[lane]),
                                ", ".join(str(path) for path, _, _
                                          in by_lane[lane])))
    for path, record, _ in entries:
        host = record["host"]
        if not lane_host_match(record["target"],
                               emulator=record.get("emulator"),
                               os_name=host["os"], machine=host["machine"]):
            problems.append(
                "{}: host {}/{} cannot record lane {}; each lane is recorded "
                "by its own host's record".format(
                    path, host["os"], host["machine"], record["target"]))
    if len({record["commit"] for _, record, _ in entries}) > 1:
        problems.append("records mix commits: {}".format(", ".join(
            "{} at {}".format(record["target"], record["commit"])
            for _, record, _ in entries)))
    if commit is not None:
        stale = ["{} at {}".format(record["target"], record["commit"])
                 for _, record, _ in entries if record["commit"] != commit]
        if stale:
            problems.append("records are stale against release commit {}: "
                            "{}".format(commit, ", ".join(stale)))
    toolchains = {(record["toolchain"]["channel"], record["toolchain"]["rustc"])
                  for _, record, _ in entries}
    if len(toolchains) > 1:
        problems.append("records mix toolchains: {}".format(", ".join(
            "{} on {} ({})".format(record["target"],
                                   record["toolchain"]["channel"],
                                   record["toolchain"]["rustc"])
            for _, record, _ in entries)))
    if problems:
        raise ValueError("; ".join(problems))

    records = {lane: by_lane[lane][0] for lane in lanes}
    statuses = {lane: lane_gate_statuses(records[lane][1]) for lane in lanes}
    rows_by_lane = {}
    for lane in lanes:
        # A per-host gate must pass on every lane; any other gate must pass
        # on every lane that ran it.
        rows = [name for name, gate in GATES.items()
                if statuses[lane][name] != "pass"
                and (gate.get("per_host")
                     or statuses[lane][name] != "not run")]
        if own_platform_run(records[lane][1]).get("status") != "recorded":
            rows.append(lane)
        rows_by_lane[lane] = rows
    unrun = [name for name, gate in GATES.items()
             if not gate.get("per_host")
             and all(statuses[lane][name] == "not run" for lane in lanes)]
    closed = not unrun and not any(rows_by_lane.values())
    lane_documents = []
    for lane in lanes:
        path, record, digest = records[lane]
        run = own_platform_run(record)
        observation = run.get("observation") or {}
        lane_documents.append({
            "target": lane,
            "runner": run.get("runner"),
            "record": str(path),
            "sha256": digest,
            "recorded_utc": record.get("recorded_utc"),
            "host": record["host"],
            "emulator": record.get("emulator"),
            "status": run.get("status"),
            "observation": {
                "command": observation.get("command"),
                "exit": observation.get("exit"),
                "elapsed_ms": observation.get("elapsed_ms"),
            },
            "gates_elapsed_ms": sum(
                row.get("elapsed_ms") or 0.0
                for gate in record.get("gates") or []
                for row in gate.get("commands") or []),
            "expected_skips": record["expected_skips"],
        })
    first = records[lanes[0]][1]
    return {
        "schema": CLOSURE_SCHEMA,
        "commit": first["commit"],
        "toolchain": {"channel": first["toolchain"]["channel"],
                      "rustc": first["toolchain"]["rustc"]},
        "status": "closed" if closed else "open",
        "open_rows": rows_by_lane,
        "unrun_gates": unrun,
        "gates": [{
            "name": name,
            "per_host": bool(gate.get("per_host")),
            "hosts": {lane: statuses[lane][name] for lane in lanes},
        } for name, gate in GATES.items()],
        "lanes": lane_documents,
    }


def load_record(path):
    data = Path(path).read_bytes()
    record = json.loads(data.decode("utf-8"))
    if not isinstance(record, dict):
        raise ValueError("{}: a release record is a JSON object".format(path))
    return path, record, hashlib.sha256(data).hexdigest()


def command_plan(args):
    selected = available_runner()
    print("runner: {}".format(selected or "none (install mbx or Cargo)"))
    lanes = host_lanes()
    print("this host: {}/{} records {}".format(
        platform.system().lower(), platform.machine().lower(),
        ", ".join(lanes) if lanes
        else "no lane directly (linux_arm64 only under --emulator)"))
    for name, gate in GATES.items():
        if args.gate and name not in args.gate:
            continue
        marker = " [per required host]" if gate.get("per_host") else ""
        print("{}{}".format(name, marker))
        print("    capability: {}".format(gate["capability"]))
        for command in gate["commands"]:
            reason = command_unavailable(command, selected)
            print("    $ {}{}".format(
                command, "" if reason is None
                else "  [unavailable: {}]".format(reason)))
    print("required platform runs:")
    for row in PLATFORM_RUNNERS:
        hosts = ", ".join(
            "{}/{}".format(os_name, machine)
            for os_name, machine in row["hosts"])
        emulator = "; or any host under a named emulator" \
            if row["target"] == "linux_arm64" else ""
        print("    {} — {} [{}]: {}{}".format(
            row["target"], row["runner"], row["execution"], hosts, emulator))
    return 0


def command_run(args):
    gate_names = args.gate or list(GATES)
    unknown = [name for name in gate_names if name not in GATES]
    if unknown:
        raise ValueError("unknown gate(s): {}".format(", ".join(unknown)))
    if args.emulator and args.target != "linux_arm64":
        raise ValueError(
            "--emulator is permitted only on the linux_arm64 row; every other"
            " runner requires direct execution")
    if not lane_host_match(args.target, emulator=args.emulator):
        raise ValueError(
            "host {}/{} cannot record lane {}: the contract requires direct "
            "execution on the matching host".format(
                platform.system().lower(), platform.machine().lower(),
                args.target))
    if not args.native_execution:
        raise ValueError(
            "--native-execution is required: a runner row is only recorded "
            "with evidence of directly executing the emitted programs")
    if args.limit is not None and args.limit < 0:
        raise ValueError("--limit must not be negative")
    expected_skips = [parse_expected_skip(entry)
                      for entry in args.expect_skip]
    undeclared = sorted({skip["gate"] for skip in expected_skips}
                        - set(gate_names))
    if undeclared:
        raise ValueError("--expect-skip names gate(s) this run does not "
                         "select: {}".format(", ".join(undeclared)))
    selected = available_runner()
    if selected is None:
        raise ValueError("Install mbx (preferred) or Cargo to run gates")
    clean = worktree_clean(excluded=[args.records_dir])
    filters = environment_filters(os.environ)
    if not clean:
        print("warning: the checkout has uncommitted changes; every gate "
              "stays open", flush=True)
    for name, value in filters.items():
        print("warning: inherited {}={!r} filters the run; every gate stays "
              "open".format(name, value), flush=True)
    log_dir = args.log_dir or LOGS_DIR / "{}__{}__{}".format(
        args.target, commit_sha()[:10],
        datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ"))
    results = {}
    for name in gate_names:
        results[name] = run_gate_commands(
            name, GATES[name]["commands"], selected, timeout=args.timeout,
            limit=args.limit, log_dir=log_dir)
    print("$ {}".format(args.native_execution), flush=True)
    observation = run_command(args.native_execution, selected,
                              timeout=args.timeout,
                              log_path=Path(log_dir) / "native-execution.log")
    print("    {}{}".format(
        "exit {} in {:.0f} ms".format(observation["exit"],
                                      observation["elapsed_ms"])
        if observation["exit"] is not None
        else "not complete: {}".format(observation["incomplete"]),
        "" if observation_recorded(observation)
        else " — lane {} open (observation failed)".format(args.target)),
        flush=True)
    record = build_record(args.target, args.emulator, observation, gate_names,
                          results, expected_skips, selected, clean, filters)
    path = write_record(record, args.records_dir)
    lane = own_platform_run(record)
    print("logs: {}".format(display_path(log_dir)))
    print("record: {} (lane {})".format(path, lane["status"]))
    print("record closure: {} (open rows: {}); join the four lanes with "
          "`close`".format(
              record["closure"]["status"],
              ", ".join(record["closure"]["open_rows"]) or "none"))
    return 0


def command_check(args):
    for path in args.record:
        _, record, _ = load_record(path)
        validate_record(record, path)
        print("{}: valid ({})".format(path, record["closure"]["status"]))
    return 0


def command_close(args):
    if args.commit is not None and not COMMIT.match(args.commit):
        raise ValueError("--commit must be a full lowercase sha")
    closure = close_records([load_record(path) for path in args.record],
                            commit=args.commit)
    text = json.dumps(closure, indent=2) + "\n"
    if args.output is None:
        sys.stdout.write(text)
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
        print("closure: {} at {} ({})".format(
            closure["status"], closure["commit"], args.output))
        for lane, rows in closure["open_rows"].items():
            if rows:
                print("    {} open: {}".format(lane, ", ".join(rows)))
        if closure["unrun_gates"]:
            print("    no lane ran: {}".format(
                ", ".join(closure["unrun_gates"])))
    # Success means a closed release; an open closure is a valid document
    # that still fails the release.
    return 0 if closure["status"] == "closed" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    plan = commands.add_parser(
        "plan", help="print the gate manifest, this host's lane and the "
        "commands this host cannot run")
    plan.add_argument("--gate", action="append", default=[],
                      choices=list(GATES), metavar="GATE")
    plan.set_defaults(handler=command_plan)

    run = commands.add_parser("run", help="run gates and write a record")
    run.add_argument("--target", required=True,
                     choices=[row["target"] for row in PLATFORM_RUNNERS],
                     help="the required platform row this record covers")
    run.add_argument("--emulator",
                     help="emulator name and version (linux_arm64 only)")
    run.add_argument("--gate", action="append", default=[],
                     choices=list(GATES), metavar="GATE",
                     help="run only these gates (default: all six)")
    run.add_argument("--native-execution", metavar="COMMAND",
                     help="command that directly executes emitted programs "
                     "for --target on this host (required; must exit 0, e.g. "
                     "a native differential test or an emulator invocation)")
    run.add_argument("--expect-skip", action="append", default=[],
                     dest="expect_skip", metavar="GATE|TEST|REASON",
                     help="declare an expected skip, e.g. "
                     "'RC-REPOSITORY|pkg::test|irrelevant to this runner'")
    run.add_argument("--timeout", type=float, default=None, metavar="SECONDS",
                     help="per-command ceiling; a timed-out command leaves "
                     "its gate open")
    run.add_argument("--limit", type=int, default=None, metavar="COUNT",
                     help="run at most this many commands per gate (smoke "
                     "runs); the rest are recorded as not run and the gate "
                     "stays open")
    run.add_argument("--records-dir", type=Path, default=RECORDS_DIR)
    run.add_argument("--log-dir", type=Path, default=None,
                     help="full per-command output (default: "
                     "target/release_record/<lane>__<commit>__<utc>/)")
    run.set_defaults(handler=command_run)

    check = commands.add_parser(
        "check", help="validate stored record(s) against the contract")
    check.add_argument("record", nargs="+")
    check.set_defaults(handler=command_check)

    close = commands.add_parser(
        "close", help="join one record per lane into the release closure; "
        "exits 0 only when the release is closed")
    close.add_argument("record", nargs="+",
                       help="one record for each of the four lanes")
    close.add_argument("--commit", default=None,
                       help="require every record at this commit (reject "
                       "stale records)")
    close.add_argument("--output", type=Path, default=None,
                       help="write the closure document here (default: "
                       "stdout)")
    close.set_defaults(handler=command_close)

    args = parser.parse_args()
    try:
        return args.handler(args)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print("release_record: {}".format(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
