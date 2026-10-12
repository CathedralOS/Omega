#!/usr/bin/env python3
"""Run one Omega process at a time; preserve every observation, never re-freeze."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--omega", required=True)
    parser.add_argument("--timeout", type=int, default=300)
    arguments = parser.parse_args()
    root = Path(__file__).resolve().parent
    repository = root.parents[3]
    omega = Path(arguments.omega).resolve()
    output = root / "build" / "validation"
    output.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, OMEGA_LIBRARY_ROOT=str(repository / "source/library"))
    records = []

    def observe(name, command, data=None):
        started = time.monotonic()
        result = subprocess.run(command, input=data, capture_output=True,
                                env=environment, timeout=arguments.timeout, cwd=repository)
        (output / f"{name}.stdout").write_bytes(result.stdout)
        (output / f"{name}.stderr").write_bytes(result.stderr)
        records.append({"name": name, "command": list(map(str, command)),
                        "exit": result.returncode, "elapsed_seconds": time.monotonic() - started,
                        "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                        "stderr_sha256": hashlib.sha256(result.stderr).hexdigest()})
        print(f"{name}: exit={result.returncode}", flush=True)
        return result

    failures = []
    reference = observe("reference", [sys.executable, str(root / "reference.py")])
    if reference.returncode:
        failures.append("reference disagrees with frozen oracle")
    checked = observe("check", [str(omega), "--check", "--target", "linux_x86_64", str(root / "main.omg")])
    if checked.returncode:
        failures.append("valid source did not check")
    else:
        for fixture in sorted((root / "fixtures").glob("*.txt")):
            expected = fixture.with_suffix(".stdout").read_bytes()
            status = int(fixture.with_suffix(".exit").read_text())
            result = observe(fixture.stem, [str(omega), "run", "--both", "--keep", str(root / "main.omg")], fixture.read_bytes())
            if result.returncode != status or result.stdout != expected:
                failures.append(f"{fixture.name}: native stdout/exit differs from frozen oracle")
            # Current public run --both compares Terminal and native exit only.
            # Its output bytes are native. Never claim Terminal trace equality.
            if f"interp exit: {status}".encode() not in result.stderr:
                failures.append(f"{fixture.name}: Terminal agreement not demonstrated")
    invalid = observe("intended-invalid", [str(omega), "--check", "--target", "linux_x86_64", str(root / "invalid/main.omg")])
    diagnostic = invalid.stdout + invalid.stderr
    if invalid.returncode != 1 or (root / "invalid/expected.txt").read_bytes().strip() not in diagnostic:
        failures.append("intended-invalid source did not reject at its slot precondition")
    record = {"base": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repository).decode().strip(),
              "host": platform.platform(), "binary": str(omega),
              "binary_sha256": hashlib.sha256(omega.read_bytes()).hexdigest(),
              "target": "linux_x86_64", "resources": {"omega_processes": 1, "rust_build_jobs": 4, "available_cpus": os.cpu_count(), "rust_profile": "release"},
              "oracle_manifest_sha256": hashlib.sha256((root / "fixtures/oracle_manifest.json").read_bytes()).hexdigest(),
              "source_hashes": {str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
                                for path in sorted(root.rglob("*.omg")) if "build" not in path.relative_to(root).parts},
              "records": records, "failures": failures,
              "terminal_scope": "Public run --both compares exit status; stdout oracle verifies native bytes."}
    (output / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    for failure in failures:
        print(f"FAIL: {failure}")
    return bool(failures)

if __name__ == "__main__":
    sys.exit(main())
