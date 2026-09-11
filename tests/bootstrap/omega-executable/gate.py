"""Compile ordinary Omega source through the selected Epsilon implementation."""

import argparse
import hashlib
import os
from pathlib import Path
import signal
import struct
import subprocess
import time


def run(command, payload, limit, label):
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
        raise SystemExit(f"{label}: timed out; no compiler judgment")
    print(f"{label}: status {process.returncode}, {time.monotonic() - started:.3f}s", flush=True)
    return process.returncode, output, errors


def evaluate(directory, program, sealed_input, limit, label):
    code, output, errors = run([str(directory / "evaluator.exe")],
                              struct.pack("<I", len(program)) + program + sealed_input,
                              limit, label)
    if code != 0 or errors:
        raise SystemExit(f"{label}: evaluator failure {code}, {output[:40].hex()}, {errors!r}")
    return output


def main():
    arguments = argparse.ArgumentParser()
    arguments.add_argument("directory", type=Path)
    arguments.add_argument("adapter", type=Path)
    arguments.add_argument("--source", type=Path)
    arguments.add_argument("--expect", type=int)
    arguments.add_argument("--expect-compile", type=int, choices=range(5), default=0)
    arguments.add_argument("--controls", action="store_true")
    options = arguments.parse_args()
    directory = options.directory.resolve()
    gate = Path(__file__).resolve().parent
    if options.controls and (options.source or options.expect is not None or options.expect_compile):
        arguments.error("--controls cannot be combined with source or expected-outcome options")
    expected_exit = options.expect if options.expect is not None else int((gate / "expected.txt").read_text())
    source = b"" if options.controls else (options.source or gate / "program.omg").read_bytes()
    limit = int(os.environ.get("OMEGA_EXECUTABLE_OBSERVATION_SECONDS", "14400"))
    if limit <= 0:
        raise SystemExit("OMEGA_EXECUTABLE_OBSERVATION_SECONDS must be positive")
    subject = (directory / "epsilon_compiler.delta").read_bytes() + options.adapter.read_bytes()
    request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject)) + subject
    receipt = evaluate(directory, (directory / "delta_compiler.gamma").read_bytes(),
                       request, 300, "Epsilon receipt reconstruction")
    if hashlib.sha256(receipt).hexdigest() != "dd4985c0eb6e1f30bc2178f90dd30e25ae7b842fb544137f606a44e622000f22":
        raise SystemExit("Epsilon execution receipt differs from the selected gate identity")
    entry = gate / ("controls.epsilon" if options.controls else "main.epsilon")
    customer = (directory / "omega_compiler.epsilon").read_bytes() + entry.read_bytes()
    print(f"Compiler customer: {len(customer)} bytes, sha256 {hashlib.sha256(customer).hexdigest()}", flush=True)
    print(f"Omega source: {len(source)} bytes, sha256 {hashlib.sha256(source).hexdigest()}", flush=True)
    observation = evaluate(directory, receipt,
                           struct.pack("<I", len(customer)) + customer + source,
                           limit, "Omega source to Alpha tape")
    if options.controls:
        expected_tape = bytes.fromhex("13 0b00000000000000 00 00 01 00 2a00000000000000 14")
        if observation != b"\x00\x00\x00\x00\x00C" + expected_tape:
            raise SystemExit(f"Compiler controls failed: {observation[:40].hex()}")
        print("PASS: 13 compiler invocations, literal bounds, entry selection, refusals and reset")
        observation = observation[:5] + observation[6:]
    expected_prefix = b"\x00" + struct.pack("<i", options.expect_compile)
    if observation[:5] != expected_prefix:
        raise SystemExit(f"Unexpected compiler outcome: {observation[:40].hex()}")
    if options.expect_compile:
        if observation != expected_prefix:
            raise SystemExit("Failed compilation published a partial tape")
        print("Expected compiler refusal with no artifact", flush=True)
        return
    tape = observation[5:]
    if not tape:
        raise SystemExit("Successful compilation emitted no tape")
    temporary = directory / "program.tape.pending"
    temporary.write_bytes(tape)
    os.replace(temporary, directory / "program.tape")
    code, output, errors = run(["sh", str(gate / "execute.sh"), str(directory)], b"", 30,
                               "Emitted Alpha program")
    if code != expected_exit or output or errors:
        raise SystemExit(f"Program result {code}, stdout {output!r}, stderr {errors!r}; expected {expected_exit}")
    print(f"PASS: {options.source or gate / 'program.omg'} -> {directory / 'program.tape'} -> exit {code}")


if __name__ == "__main__":
    main()