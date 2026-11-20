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
    require_identity("Epsilon", epsilon, 613769,
                     "c5392b0a71cbe58de5090fb888b379fa5cb3b4b598ba38d6f1a568e326ffa0ff")
    require_identity("execution adapter", adapter, 2150,
                     "ab79416b4278d3b132a4fd432485f1b90e868c0549af96e604e2e8da570d1baa")
    compiler = (directory / "omega_compiler.epsilon").read_bytes()
    require_identity("D", compiler, 888824,
                     "55aed32e6f8a828649d9f01aba27cafffccab975859aef91bb3e7add720fc218")
    return compiler, epsilon


def require_receipt_identity(receipt):
    require_identity("Epsilon execution receipt", receipt, 710527,
                     "e20359359e996a32c6bff1bb3162eed091a54e0c5a9550f5b168dda8f6eea850")
