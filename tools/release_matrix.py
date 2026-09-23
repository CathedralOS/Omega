#!/usr/bin/env python3
"""Run the Rust product release matrix and write its release record.

Drives the eight named gates and the hosted platform rows of
wiki/drafts/reference/rust_compiler_completion.md (the "Release matrix" contract) from a
clean checkout, then writes a JSON release record carrying the commit, pinned
toolchain, host OS/architecture, exact commands, per-command results and elapsed
time, and the expected-skip list the closure rule requires. A skipped or failed
command leaves its gate open — never a pass; the contract's "no partial
percentage, no averaging" rule is enforced by the record, not by the runner.

Commands run as argument vectors without a shell, so the same invocation works
in PowerShell and a POSIX shell: `python tools/release_matrix.py --plan` prints
what would run on this host; `python tools/release_matrix.py --run` executes it.
`--gate NAME` (repeatable) selects a subset for a scoped row refresh.
`mbx` is used when installed, otherwise Cargo, matching the AGENTS.md wrapper
rule; the formatting leg runs this interpreter on the portable `tools/fmt.py`
route the contract names. The host row records the native
observation only when this host matches a required runner; the remaining rows
stay open rather than silently passing.
"""

import argparse
import json
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

MBX = "mbx"
# The contract spells the formatting leg `python tools/fmt.py --check`; the
# leading "python" resolves to the running interpreter, the same way "mbx"
# resolves to the installed wrapper or Cargo.
PYTHON = "python"

# Gate table transcribed from wiki/drafts/reference/rust_compiler_completion.md
# "Release matrix". Commands keep the contract's exact invocations; the leading
# "mbx" resolves to the installed mbx or falls back to cargo at run time.
PACKAGE_AND_BUILD_PACKAGES = [
    "build-declarations", "build-evaluation", "package-compilation",
    "package-source", "resolver-execution", "package-evidence",
    "package-manager",
]

GATES = {
    "RC-REPOSITORY": {
        "capability": "The pinned toolchain formats, lints, type-checks, and "
                      "preserves architectural dependency boundaries.",
        "commands": [
            [PYTHON, "tools/fmt.py", "--check"],
            [MBX, "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
            [MBX, "nextest", "run", "-p", "omega-architecture-test",
             "--all-targets", "--no-fail-fast"],
            [MBX, "check", "--workspace", "--all-targets"],
            [MBX, "nextest", "run", "--workspace", "--lib", "--no-fail-fast"],
        ],
    },
    "RC-SOURCE-SEMANTICS": {
        "capability": "Every accepted positive fixture reaches its promised "
                      "checked or product stage; every negative fixture "
                      "rejects; individual semantic integration tests pass.",
        "commands": [
            [MBX, "nextest", "run", "-p", "compiler", "--all-targets",
             "--no-fail-fast"],
        ],
    },
    "RC-PCC-REPLAY": {
        "capability": "Requested artifact/.proof pairs round-trip; hostile or "
                      "substituted evidence rejects before PCC-required "
                      "interpretation or lowering.",
        "commands": [
            [MBX, "nextest", "run", "-p", "checked-trees-to-lowered-psi",
             "-p", "terminal-codec", "-p", "terminal-verifier",
             "-p", "terminal-interpreter",
             "-p", "terminal-psi-to-abstract-operations", "--no-fail-fast"],
            [MBX, "test", "--doc", "-p", "checked-trees-to-lowered-psi",
             "-p", "terminal-codec", "-p", "terminal-verifier",
             "-p", "terminal-interpreter",
             "-p", "terminal-psi-to-abstract-operations"],
        ],
    },
    "RC-PORTABLE-PSI": {
        "capability": "One process publishes a complete source-free Terminal "
                      "Psi envelope and exits; a second process reconstructs, "
                      "verifies, and interprets it using newly supplied "
                      "authority.",
        "commands": [
            [MBX, "nextest", "run", "-p", "compiler", "--test", "canary_suite",
             "--no-fail-fast", "--no-tests", "fail", "-E",
             "test(=portable_terminal_reload::"
             "portable_terminal_product_reloads_across_process_boundary)"],
        ],
    },
    "RC-BUILD-AND-PACKAGES": {
        "capability": "Build declarations, immutable inputs, package "
                      "identities, reviewed evidence, resolution, and "
                      "compilation handoff agree without path or name "
                      "inference.",
        "commands": [
            [MBX, "nextest", "run"] +
            [part for package in PACKAGE_AND_BUILD_PACKAGES
             for part in ("-p", package)] + ["--no-fail-fast"],
            [MBX, "test", "--doc"] +
            [part for package in PACKAGE_AND_BUILD_PACKAGES
             for part in ("-p", package)],
            [MBX, "nextest", "run", "-p", "compiler",
             "--test", "build_config_granted",
             "--test", "build_log_facet",
             "--test", "build_target_activation",
             "--test", "checked_build_machine_identity",
             "--test", "evaluated_via_binding",
             "--test", "package_compilation_inputs",
             "--no-fail-fast"],
        ],
    },
    "RC-NATIVE-MATRIX": {
        "capability": "Each hosted target produces independently validated "
                      "machine code, object/image bytes, ABI behavior, "
                      "provider settlement, and observable execution on its "
                      "matching host.",
        "commands": [
            [MBX, "nextest", "run", "-p", "omega-native-differential-test",
             "--all-targets", "--no-fail-fast"],
        ],
    },
    "RC-DIAGNOSTICS": {
        "capability": "Rejected source and failed product admission report "
                      "stable, actionable diagnostics rather than panics, "
                      "silent fallback, or accidental acceptance.",
        "commands": [
            [MBX, "nextest", "run", "-p", "compiler", "--test", "canary_suite",
             "--no-fail-fast", "--no-tests", "fail", "-E",
             "test(=proof_and_float_suites::proof_and_domain_canaries::"
             "fail_canaries_reject_with_expected_diagnostic_fragment)"],
        ],
    },
    "RC-REPRESENTATIVE-PROGRAMS": {
        "capability": "Every maintained sample reaches checked semantics; "
                      "every sample with an authored host entry reaches its "
                      "native product; every documented deterministic "
                      "exit/output oracle passes.",
        "commands": [
            [MBX, "nextest", "run", "-p", "compiler", "--test",
             "samples_compile", "--no-fail-fast"],
        ],
    },
}

# The contract's required platform runs: (system, machine) aliases -> product
# identity. Each row needs direct execution of the emitted product on its
# matching host; this host's row is the only one a local run can close.
PLATFORM_ROWS = {
    ("linux", ("x86_64", "amd64")): {
        "product": "linux_x86_64",
        "observation": "direct execution of emitted ELF x86-64 programs",
    },
    ("linux", ("aarch64", "arm64")): {
        "product": "linux_arm64",
        "observation": "direct execution of emitted ELF AArch64 programs "
                       "(emulation only when the record names the emulator)",
    },
    ("darwin", ("arm64", "aarch64")): {
        "product": "macos_arm64",
        "observation": "direct execution of emitted Mach-O AArch64 programs",
    },
    ("windows", ("x86_64", "amd64")): {
        "product": "windows_x86_64",
        "observation": "direct execution of emitted PE x86-64 programs",
    },
}

TOOL_REQUIREMENTS = {
    "nextest": ["cargo-nextest >= 0.9.140"],
}


def repo_root():
    out = subprocess.check_output(
        ["git", "rev-parse", "--show-toplevel"], text=True).strip()
    return Path(out).resolve()


def pinned_toolchain(root):
    toolchain = (root / "rust-toolchain.toml")
    channel = None
    if toolchain.is_file():
        for line in toolchain.read_text(encoding="utf-8").splitlines():
            stripped = line.strip()
            if stripped.startswith("channel"):
                channel = stripped.split("=", 1)[1].strip().strip('"').strip("'")
    rustc = None
    resolved = shutil.which("rustc")
    if resolved:
        rustc = subprocess.check_output(
            [resolved, "--version"], text=True).strip()
    return {"channel": channel, "rustc": rustc}


def host_row():
    system = platform.system().lower()
    machine = platform.machine().lower()
    for (os_name, machines), row in PLATFORM_ROWS.items():
        if system == os_name and machine in machines:
            return dict(row)
    return None


def resolve(argv, runner):
    if argv[0] == MBX:
        return [runner] + argv[1:]
    if argv[0] == PYTHON:
        return [sys.executable] + argv[1:]
    return list(argv)


def command_supported(argv, runner):
    # This interpreter is running, so the formatting leg is always available.
    if argv[0] == PYTHON:
        return True
    if argv[0] == MBX and runner is None:
        return False
    if "nextest" in argv:
        probe = runner if argv[0] == MBX else "cargo"
        return subprocess.run(
            [probe, "nextest", "--version"], check=False,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    return True


def gate_commands(runner):
    plan = {}
    for name, gate in GATES.items():
        commands = []
        for argv in gate["commands"]:
            commands.append({
                "argv": resolve(argv, runner),
                "supported": command_supported(argv, runner),
            })
        plan[name] = {"capability": gate["capability"], "commands": commands}
    return plan


def run_gate(root, name, gate_plan, log_dir, timeout, limit=None):
    results = []
    verdict = "pass"
    for index, command in enumerate(gate_plan["commands"]):
        argv = command["argv"]
        log_path = log_dir / f"{name}-{index}.log"
        if limit is not None and index >= limit:
            results.append({"argv": argv, "exit": None,
                            "skip": "beyond --limit"})
            verdict = "open"
            continue
        if not command["supported"]:
            results.append({"argv": argv, "exit": None,
                            "skip": "runner or cargo-nextest unavailable"})
            verdict = "open"
            continue
        started = time.monotonic()
        try:
            process = subprocess.run(argv, cwd=root, check=False,
                                     stdout=subprocess.PIPE,
                                     stderr=subprocess.STDOUT,
                                     timeout=timeout)
            output = process.stdout
            exit_code = process.returncode
        except subprocess.TimeoutExpired as expired:
            # The contract: a timeout leaves the row open, never a pass.
            output = expired.stdout or b""
            exit_code = None
        elapsed = time.monotonic() - started
        log_path.write_bytes(output)
        result = {"argv": argv, "exit": exit_code,
                  "elapsed_seconds": round(elapsed, 3),
                  "log": str(log_path)}
        if exit_code is None:
            result["skip"] = "timeout" if timeout else "no exit"
            verdict = "open"
        elif exit_code != 0:
            verdict = "open"
        results.append(result)
    return verdict, results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", action="store_true",
                        help="print the gate plan as JSON without running it")
    parser.add_argument("--run", action="store_true",
                        help="execute the selected gates and write the record")
    parser.add_argument("--gate", action="append", default=[],
                        help="restrict to these gate names (repeatable)")
    parser.add_argument("--record", default=None,
                        help="release record path "
                             "(default target/release_matrix/<host>.json)")
    parser.add_argument("--timeout", type=float, default=None,
                        help="per-command timeout seconds; a timeout leaves "
                             "the gate open")
    parser.add_argument("--limit", type=int, default=None,
                        help="run at most this many commands per gate "
                             "(scoped smoke runs; the gate is not closed by "
                             "a limited run)")
    args = parser.parse_args()

    try:
        root = repo_root()
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"release_matrix: not inside a checkout: {error}",
              file=sys.stderr)
        return 2

    runner = shutil.which("mbx") or shutil.which("cargo")
    row = host_row()
    commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    toolchain = pinned_toolchain(root)
    plan = gate_commands(runner)

    selected = args.gate or list(GATES)
    unknown = [name for name in selected if name not in GATES]

    if unknown:
        print(f"release_matrix: unknown gates {unknown}; "
              f"known: {sorted(GATES)}", file=sys.stderr)
        return 2

    record = {
        "contract": "wiki/drafts/reference/rust_compiler_completion.md#release-matrix",
        "commit": commit,
        "toolchain": toolchain,
        "host": {"system": platform.system(), "machine": platform.machine()},
        "runner": runner,
        "platform_row": row,
        "gates": plan,
    }

    if args.plan or not args.run:
        print(json.dumps({**record, "selected_gates": selected}, indent=2))
        return 0

    if runner is None:
        print("release_matrix: install mbx (preferred) or Cargo, and "
              "cargo-nextest; no command ran", file=sys.stderr)
        return 2

    record_path = Path(args.record) if args.record else (
        root / "target" / "release_matrix" /
        f"record-{platform.system().lower()}-{platform.machine().lower()}.json")
    log_dir = record_path.parent / (record_path.stem + "-logs")
    log_dir.mkdir(parents=True, exist_ok=True)

    outcomes = {}
    skips = []
    for name in selected:
        verdict, results = run_gate(root, name, plan[name], log_dir,
                                    args.timeout, args.limit)
        outcomes[name] = {"verdict": verdict, "commands": results}
        for result in results:
            if "skip" in result:
                skips.append({"gate": name, "command": result["argv"],
                              "reason": result["skip"]})
        print(f"{name}: {verdict}", flush=True)

    rows = {entry["product"]: "open: runner unavailable on this host"
            for entry in PLATFORM_ROWS.values()}
    if row is not None:
        # The contract closes a host row on the native matrix plus the source
        # and samples gates all passing on that same host.
        native_ok = all(outcomes.get(name, {}).get("verdict") == "pass"
                        for name in (
                            "RC-NATIVE-MATRIX", "RC-SOURCE-SEMANTICS",
                            "RC-REPRESENTATIVE-PROGRAMS"))
        rows[row["product"]] = "pass" if native_ok else "open"
    record.update({
        "results": outcomes,
        "platform_rows": rows,
        "expected_skips": skips,
    })
    record_path.parent.mkdir(parents=True, exist_ok=True)
    record_path.write_text(json.dumps(record, indent=2) + "\n",
                           encoding="utf-8")
    print(f"release record: {record_path}")

    # A gate row closes only on an outright pass; any open or failed gate is a
    # nonzero exit so an incomplete run cannot read as success. Host rows for
    # runners this host cannot execute stay "open" in the record.
    return 0 if all(v["verdict"] == "pass" for v in outcomes.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
