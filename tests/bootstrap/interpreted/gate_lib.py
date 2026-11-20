"""Shared gate machinery for the tests/bootstrap/interpreted omega-* gates: a watchdog
subprocess runner, the sealed evaluator invocation, and the bound-identity
check. Gates import this file from their own directory parent."""

import hashlib
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import time


def run(command, payload, limit, label, timed_out):
    started = time.monotonic()
    print(f"{label}: started, {limit}s watchdog", flush=True)
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        output, errors = process.communicate(payload, timeout=limit)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], check=False)
        else:
            os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"{label}: timed out; {timed_out}")
    print(f"{label}: status {process.returncode}, {time.monotonic() - started:.3f}s", flush=True)
    return process.returncode, output, errors


def evaluate(directory, program, sealed_input, limit, label, timed_out):
    code, output, errors = run([str(directory / "evaluator.exe")],
                              struct.pack("<I", len(program)) + program + sealed_input,
                              limit, label, timed_out)
    if code != 0 or errors:
        raise SystemExit(f"{label}: evaluator failure {code}, {output[:40].hex()}, {errors!r}")
    return output


def require_identity(label, source, length, digest):
    actual = hashlib.sha256(source).hexdigest()
    if len(source) != length or actual != digest:
        raise SystemExit(
            f"{label} identity changed: {len(source)} bytes, SHA-256 {actual}")


def env_seconds(name, default):
    value = int(os.environ.get(name, default))
    if value <= 0:
        raise SystemExit(f"{name} must be positive")
    return value


def require_rung_identities(directory, adapter):
    """Bound Epsilon compiler, execution adapter, and D compiler identities.
    Return the checked D and Epsilon bytes so callers frame exactly that
    validated evaluator subject; callers keep their per-entry checks."""
    epsilon = (directory / "epsilon_compiler.delta").read_bytes()
    require_identity("Epsilon", epsilon, 618016,
                     "02be715b60938be45bd89984df6bfb197e1b46310435feae77b9b6271c4a0cd0")
    require_identity("execution adapter", adapter, 2150,
                     "ab79416b4278d3b132a4fd432485f1b90e868c0549af96e604e2e8da570d1baa")
    compiler = (directory / "omega_compiler.epsilon").read_bytes()
    require_identity("D", compiler, 868532,
                     "ab724ba5aac153ed442dd4d3346f78ddadfffeebd78c39151d73c573fa00eb21")
    return compiler, epsilon


def require_receipt_identity(receipt):
    require_identity("Epsilon execution receipt", receipt, 722080,
                     "cd7ace8a00a6583ce22af16108b8bd003ac18d1e56a9770580682d188daaf80a")
