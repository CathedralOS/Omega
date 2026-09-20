#!/usr/bin/env python3
"""Run the repository's documented full-baseline gates as one command.

Every gate is the exact command named by AGENTS.md's "Full baseline" section;
this runner adds nothing and skips nothing. It prefers `mbx` and falls back to
Cargo, uses `cargo fmt` directly as documented, streams each gate's own output
in order, and keeps going after a failure so the closing summary reports every
gate's verdict in one pass. `--plan` prints the resolved JSON without running.

The run is checkout-local evidence: it neither caches results nor grants a
landing reservation, and the summary records which host ran each gate for the
platform-support matrix. Selection policy and the corpus filter belong to
AGENTS.md; see tools/testing.md.
"""

import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time

# gate name -> argv with the runner placeholder resolved per host. `fmt` is a
# documented cargo-only command; the rest take the resolved Rust runner.
def baseline_commands(runner):
    return [
        ("fmt", ["cargo", "fmt", "--all", "--", "--check"]),
        ("clippy", [runner, "clippy", "--workspace", "--all-targets", "--",
                    "-D", "warnings"]),
        ("architecture", [runner, "nextest", "run", "-p", "omega-architecture-test",
                          "--all-targets", "--no-fail-fast"]),
        ("canary-corpus-audit",
         [runner, "nextest", "run", "-p", "compiler", "--test", "canary_suite",
          "--no-fail-fast", "--no-tests", "fail", "-E",
          "test(=surface_and_targets::retired_domain_when_surface_is_absent_from_authored_corpus)"]),
        ("check", [runner, "check", "--workspace", "--all-targets"]),
        ("libraries", [runner, "nextest", "run", "--workspace", "--lib",
                       "--no-fail-fast"]),
    ]


def resolve_runner():
    runner = shutil.which("mbx") or shutil.which("cargo")
    if not runner:
        raise ValueError("Install mbx (preferred) or Cargo, and cargo-nextest")
    return runner


def host_label():
    return f"{platform.system().lower()} {platform.machine()}"


def run_gates(root, gates, host):
    results = []
    for name, command in gates:
        print(f"=== baseline gate: {name} ({host}) ===", flush=True)
        started = time.monotonic()
        result = subprocess.run(command, cwd=root, check=False)
        elapsed = time.monotonic() - started
        results.append((name, command, result.returncode, elapsed))
    return results


def report(results, host, runner):
    print(f"=== baseline gate summary ({host}, runner {runner}) ===")
    for name, command, returncode, elapsed in results:
        verdict = "pass" if returncode == 0 else f"FAIL ({returncode})"
        print(f"  {name:<22} {verdict:<10} {elapsed:8.1f}s  {' '.join(command)}")
    failed = [name for name, _, returncode, _ in results if returncode != 0]
    print(f"baseline_gate: {len(results) - len(failed)}/{len(results)} gates passed"
          + (f"; failed: {', '.join(failed)}" if failed else ""))
    return 1 if failed else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", action="store_true",
                        help="Print the resolved gate plan as JSON without running")
    parser.add_argument("--only", metavar="NAME[,NAME...]",
                        help="Run only the named gates (see --plan for names)")
    args = parser.parse_args()
    try:
        runner = resolve_runner()
        gates = baseline_commands(runner)
        if args.only:
            wanted = {name.strip() for name in args.only.split(",") if name.strip()}
            unknown = wanted - {name for name, _ in gates}
            if unknown:
                raise ValueError(f"unknown gate(s): {', '.join(sorted(unknown))}")
            gates = [(name, command) for name, command in gates if name in wanted]
        host = host_label()
        plan = {"runner": runner, "host": host,
                "gates": [{"name": name, "command": command}
                          for name, command in gates]}
        if args.plan:
            print(json.dumps(plan, indent=2))
            return 0
        root = Path(subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=Path.cwd()).decode("utf-8").strip()).resolve()
        results = run_gates(root, gates, host)
        return report(results, host, runner)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"baseline_gate: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
