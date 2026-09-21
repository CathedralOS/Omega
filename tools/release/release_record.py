#!/usr/bin/env python3
"""Release-record substrate for the Rust compiler completion matrix.

`wiki/drafts/rust_compiler_completion.md` defines eight named gates and four
required platform runs, and requires the release record to contain the commit,
pinned Rust toolchain, host OS and architecture, commands, results, elapsed
time, and the exact list of expected skips. This tool is that record's
substrate: it owns the gate manifest verbatim, runs a gate's command block on
one host, and writes one versioned JSON record per runner row under
`records/` so a row's evidence survives the machine that produced it.

The harness records outcomes; it does not judge the matrix. A red gate is
recorded as `fail` and the closure stays `open` — the substrate never turns a
failed or missing row into a pass. `check` re-validates a stored record
against the contract so a hand-edited or truncated record cannot read as
closed.

Requirements: Python 3.9+ and Git. Standard library only; identical under
PowerShell on Windows and sh on macOS/Linux. Commands run through `mbx` when
it is installed and through Cargo otherwise, per AGENTS.md.

    python3 tools/release/release_record.py plan
    python3 tools/release/release_record.py run --target linux_x86_64 \
        --native-execution "mbx nextest run -p omega-native-differential-test"
    python3 tools/release/release_record.py run --target linux_arm64 \
        --emulator "qemu-aarch64 9.0.0" --gate RC-NATIVE-MATRIX \
        --native-execution "qemu-aarch64 <emitted-elf>"
    python3 tools/release/release_record.py check records/<file>.json

A runner row is only `recorded` when the record carries a passing
`--native-execution` observation — direct execution of the emitted programs —
and `run` refuses a lane this host cannot execute (the `linux_arm64` lane
also accepts any host under a named emulator).
"""

import argparse
import json
import platform
import re
import shlex
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

SCHEMA = "omega-release-record/1"

ROOT = Path(__file__).resolve().parents[2]
TOOLCHAIN_FILE = ROOT / "rust-toolchain.toml"
RECORDS_DIR = Path(__file__).resolve().parent / "records"

# The eight gates of the release matrix, verbatim from
# wiki/drafts/rust_compiler_completion.md. Each gate runs its command block
# through the resolved `mbx`/`cargo` wrapper. A test in
# tools/tests/test_release_record.py pins this manifest against the document
# so the substrate cannot silently diverge from the contract it records.
GATES = {
    "RC-REPOSITORY": {
        "capability": "The pinned toolchain formats, lints, type-checks, and "
                      "preserves architectural dependency boundaries.",
        "commands": [
            "cargo fmt --all -- --check",
            "mbx clippy --workspace --all-targets -- -D warnings",
            "mbx nextest run -p omega-architecture-test --all-targets --no-fail-fast",
            "mbx check --workspace --all-targets",
            "mbx nextest run --workspace --lib --no-fail-fast",
        ],
    },
    "RC-SOURCE-SEMANTICS": {
        "capability": "Every accepted positive fixture reaches its promised "
                      "checked or product stage; every negative fixture "
                      "rejects; individual semantic integration tests pass.",
        "commands": [
            "mbx nextest run -p compiler --all-targets --no-fail-fast",
        ],
    },
    "RC-PCC-REPLAY": {
        "capability": "Requested artifact/.proof pairs round-trip; hostile or "
                      "substituted evidence rejects before PCC-required "
                      "interpretation or lowering. Ordinary output still "
                      "checks without publishing PCC.",
        "commands": [
            "mbx nextest run -p checked-trees-to-lowered-psi "
            "-p terminal-codec -p terminal-verifier -p terminal-interpreter "
            "-p terminal-psi-to-abstract-operations --no-fail-fast",
            "mbx test --doc -p checked-trees-to-lowered-psi "
            "-p terminal-codec -p terminal-verifier -p terminal-interpreter "
            "-p terminal-psi-to-abstract-operations",
        ],
    },
    "RC-PORTABLE-PSI": {
        "capability": "One process publishes a complete source-free Terminal "
                      "Psi envelope and exits; a second process reconstructs, "
                      "verifies, and interprets it using newly supplied "
                      "authority.",
        "commands": [
            "mbx nextest run -p compiler --test canary_suite --no-fail-fast "
            "--no-tests fail "
            "-E 'test(=portable_terminal_reload::"
            "portable_terminal_product_reloads_across_process_boundary)'",
        ],
    },
    "RC-BUILD-AND-PACKAGES": {
        "capability": "Build declarations, immutable inputs, package "
                      "identities, reviewed evidence, resolution, and "
                      "compilation handoff agree without path/name inference "
                      "or hidden ambient mutation.",
        "commands": [
            "mbx nextest run -p build-declarations -p build-evaluation "
            "-p package-compilation -p package-source -p resolver-execution "
            "-p package-evidence -p package-manager --no-fail-fast",
            "mbx test --doc -p build-declarations -p build-evaluation "
            "-p package-compilation -p package-source -p resolver-execution "
            "-p package-evidence -p package-manager",
        ],
    },
    "RC-NATIVE-MATRIX": {
        "capability": "Each hosted target produces independently validated "
                      "machine code, object/image bytes, ABI behavior, "
                      "provider settlement, and observable execution on its "
                      "matching host.",
        "commands": [
            "mbx nextest run -p omega-native-differential-test "
            "--all-targets --no-fail-fast",
        ],
        "per_host": True,
    },
    "RC-DIAGNOSTICS": {
        "capability": "Rejected source and failed product admission report "
                      "stable, actionable diagnostics rather than panics, "
                      "silent fallback, or accidental acceptance.",
        "commands": [
            "mbx nextest run -p compiler --test canary_suite --no-fail-fast "
            "--no-tests fail "
            "-E 'test(=proof_and_float_suites::"
            "fail_canaries_reject_with_expected_diagnostic_fragment)'",
        ],
    },
    "RC-REPRESENTATIVE-PROGRAMS": {
        "capability": "Every maintained sample reaches checked semantics; "
                      "every sample with an authored host entry reaches its "
                      "native product; every documented deterministic "
                      "exit/output oracle passes.",
        "commands": [
            "mbx nextest run -p compiler --test samples_compile --no-fail-fast",
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

NEXTEST_SKIPPED = re.compile(r"^\s*SKIP\s+\S+\s+(\S+)\s*$", re.M)
NEXTEST_SUMMARY = re.compile(r"(\d+) skipped")
CARGO_TEST_RESULT = re.compile(
    r"test result: \w+\. \d+ passed; \d+ failed; (\d+) ignored")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
TARGET_NAME = re.compile(r"^[a-z][a-z0-9_]*$")
# Captured output is evidence, not a log store: keep the tail bounded.
OUTPUT_TAIL_BYTES = 64 * 1024


def lane_host_match(target, emulator=None, os_name=None, machine=None):
    """Whether the given (or current) host may record the lane's row.

    Direct lanes require the contract's matching host. The `linux_arm64` row
    additionally accepts a named emulator from any host, per the contract's
    "emulation is acceptable only when the release record names the emulator
    and version" allowance.
    """
    os_name = (os_name if os_name is not None else platform.system()).lower()
    machine = (machine if machine is not None
               else platform.machine()).lower()
    hosts = {row["target"]: row["hosts"] for row in PLATFORM_RUNNERS}[target]
    if (os_name, machine) in hosts:
        return True
    return target == "linux_arm64" and bool(emulator)


def runner():
    selected = shutil.which("mbx") or shutil.which("cargo")
    if selected is None:
        raise ValueError("Install mbx (preferred) or Cargo to run gates")
    return selected


def resolve_command(text, selected_runner):
    argv = shlex.split(text)
    if argv and argv[0] == "mbx":
        argv[0] = selected_runner
    return argv


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


def run_command(command, selected_runner):
    argv = resolve_command(command, selected_runner)
    started = time.monotonic()
    completed = subprocess.run(
        argv, cwd=ROOT, capture_output=True, text=True, check=False)
    elapsed_ms = (time.monotonic() - started) * 1000.0
    output = completed.stdout + completed.stderr
    skipped_names = sorted(set(NEXTEST_SKIPPED.findall(output)))
    skipped_count = len(skipped_names)
    if not skipped_count:
        summary = NEXTEST_SUMMARY.findall(output)
        ignored = CARGO_TEST_RESULT.findall(output)
        skipped_count = (
            sum(int(value) for value in summary)
            + sum(int(value) for value in ignored))
    return {
        "command": command,
        "argv": argv,
        "exit": completed.returncode,
        "elapsed_ms": elapsed_ms,
        "skipped_tests": skipped_count,
        "skipped_names": skipped_names,
        "output_tail": output[-OUTPUT_TAIL_BYTES:],
    }


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
                 expected_skips, selected_runner):
    gates = []
    for name in gate_names:
        gate = GATES[name]
        commands = results[name]
        status = "pass" if all(row["exit"] == 0 for row in commands) else "fail"
        gates.append({
            "name": name,
            "capability": gate["capability"],
            "status": status,
            "commands": commands,
        })
    skipped_named = {
        name for gate in gates for row in gate["commands"]
        for name in row["skipped_names"]}
    unlisted = sorted(
        name for name in skipped_named
        if not any(skip["test"] == name for skip in expected_skips))
    platform_runs = [{
        "runner": row["runner"],
        "target": row["target"],
        # A lane reads recorded only with a passed direct-execution
        # observation; a red or missing probe leaves the row open.
        "status": ("recorded" if row["target"] == target
                   and observation["exit"] == 0 else "open"),
        "emulator": (emulator if row["target"] == target else None),
        "observation": (observation if row["target"] == target else None),
    } for row in PLATFORM_RUNNERS]
    open_rows = (
        [gate["name"] for gate in gates if gate["status"] != "pass"]
        + [name for name in GATES if name not in gate_names]
        + [row["target"] for row in platform_runs
           if row["status"] != "recorded"])
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
        "gates": gates,
        "platform_runs": platform_runs,
        "expected_skips": expected_skips,
        "unlisted_skips": unlisted,
        "closure": {
            "status": "open" if (open_rows or unlisted) else "closed",
            "open_rows": open_rows,
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
    gates = {gate.get("name"): gate for gate in record.get("gates") or []}
    for name, gate in GATES.items():
        if name not in gates:
            continue
        row = gates[name]
        need(row.get("status") in ("pass", "fail", "open"),
             "gate {} has invalid status".format(name))
        commands = row.get("commands") or []
        need(len(commands) == len(gate["commands"]),
             "gate {} records {} commands, contract has {}".format(
                 name, len(commands), len(gate["commands"])))
        for observed, expected in zip(commands, gate["commands"]):
            need(observed.get("command") == expected,
                 "gate {} command drifted from the contract".format(name))
            need(isinstance(observed.get("exit"), int)
                 and isinstance(observed.get("elapsed_ms"), (int, float)),
                 "gate {} command lacks exit/elapsed evidence".format(name))
            need(isinstance(observed.get("skipped_tests"), int),
                 "gate {} command lacks a skip count".format(name))
    expected_skips = record.get("expected_skips")
    need(isinstance(expected_skips, list),
         "expected_skips must be the exact recorded list")
    need(isinstance(record.get("unlisted_skips"), list)
         and not record["unlisted_skips"],
         "recorded skipped tests missing from expected_skips: {}".format(
             record.get("unlisted_skips")))
    for skip in expected_skips or []:
        gate = gates.get(skip.get("gate")) or {}
        observed = {name for row in gate.get("commands") or []
                    for name in row.get("skipped_names") or []}
        if observed:
            need(skip.get("test") in observed,
                 "expected skip {} was declared but not observed in {}".format(
                     skip.get("test"), skip.get("gate")))
    runs = {row.get("target"): row for row in record.get("platform_runs") or []}
    host = record.get("host") or {}
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
        need(isinstance(observation, dict)
             and isinstance(observation.get("command"), str)
             and observation["command"].strip()
             and isinstance(observation.get("argv"), list)
             and observation.get("exit") == 0
             and isinstance(observation.get("elapsed_ms"), (int, float)),
             "platform run {} is recorded without a passing "
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
    all_pass = all(
        gates.get(name, {}).get("status") == "pass" for name in GATES)
    all_recorded = all(
        runs.get(row["target"], {}).get("status") == "recorded"
        for row in PLATFORM_RUNNERS)
    expected_closure = "closed" if (all_pass and all_recorded
                                  and not record.get("unlisted_skips")) else "open"
    need(closure.get("status") == expected_closure,
         "closure.status {} contradicts the recorded rows (expected {})"
         .format(closure.get("status"), expected_closure))
    if problems:
        raise ValueError("{}: {}".format(path, "; ".join(problems)))


def command_plan(args):
    selected = shutil.which("mbx") or "cargo (mbx unavailable)"
    print("runner: {}".format(selected))
    for name, gate in GATES.items():
        if args.gate and name not in args.gate:
            continue
        marker = " [per required host]" if gate.get("per_host") else ""
        print("{}{}".format(name, marker))
        print("    capability: {}".format(gate["capability"]))
        for command in gate["commands"]:
            print("    $ {}".format(command))
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
    expected_skips = [parse_expected_skip(entry)
                      for entry in args.expect_skip]
    selected = runner()
    results = {}
    for name in gate_names:
        rows = []
        for command in GATES[name]["commands"]:
            print("$ {}".format(command), flush=True)
            row = run_command(command, selected)
            rows.append(row)
            print("    exit {} in {:.0f} ms{}".format(
                row["exit"], row["elapsed_ms"],
                " ({} skipped)".format(row["skipped_tests"])
                if row["skipped_tests"] else ""), flush=True)
        results[name] = rows
    print("$ {}".format(args.native_execution), flush=True)
    observation = run_command(args.native_execution, selected)
    print("    exit {} in {:.0f} ms{}".format(
        observation["exit"], observation["elapsed_ms"],
        " — lane {} open (observation failed)".format(args.target)
        if observation["exit"] != 0 else ""), flush=True)
    record = build_record(args.target, args.emulator, observation, gate_names,
                          results, expected_skips, selected)
    path = write_record(record, args.records_dir)
    lane = next(row for row in record["platform_runs"]
                if row["target"] == args.target)
    print("record: {} (lane {})".format(path, lane["status"]))
    print("closure: {} (open rows: {})".format(
        record["closure"]["status"],
        ", ".join(record["closure"]["open_rows"]) or "none"))
    return 0


def command_check(args):
    for path in args.record:
        record = json.loads(Path(path).read_text(encoding="utf-8"))
        validate_record(record, path)
        print("{}: valid ({})".format(path, record["closure"]["status"]))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)

    plan = commands.add_parser("plan", help="print the gate manifest")
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
                     help="run only these gates (default: all eight)")
    run.add_argument("--native-execution", metavar="COMMAND",
                     help="command that directly executes emitted programs "
                     "for --target on this host (required; must exit 0, e.g. "
                     "a native differential test or an emulator invocation)")
    run.add_argument("--expect-skip", action="append", default=[],
                     dest="expect_skip", metavar="GATE|TEST|REASON",
                     help="declare an expected skip, e.g. "
                     "'RC-REPOSITORY|pkg::test|irrelevant to this runner'")
    run.add_argument("--records-dir", type=Path, default=RECORDS_DIR)
    run.set_defaults(handler=command_run)

    check = commands.add_parser(
        "check", help="validate stored record(s) against the contract")
    check.add_argument("record", nargs="+")
    check.set_defaults(handler=command_check)

    args = parser.parse_args()
    try:
        return args.handler(args)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print("release_record: {}".format(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
