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
    arguments.add_argument("--controls-b", action="store_true")
    arguments.add_argument("--controls-c", action="store_true")
    arguments.add_argument("--controls-d", action="store_true")
    arguments.add_argument("--controls-e", action="store_true")
    options = arguments.parse_args()
    directory = options.directory.resolve()
    gate = Path(__file__).resolve().parent
    selected = [options.controls, options.controls_b, options.controls_c,
                options.controls_d, options.controls_e]
    if sum(1 for flag in selected if flag) > 1:
        arguments.error("controls parts are separate runs")
    if any(selected) and \
            (options.source or options.expect is not None or options.expect_compile):
        arguments.error("controls cannot be combined with source or expected-outcome options")
    expected_exit = options.expect if options.expect is not None else int((gate / "expected.txt").read_text())
    controls = any(selected)
    source = b"" if controls else (options.source or gate / "program.omg").read_bytes()
    limit = int(os.environ.get("OMEGA_EXECUTABLE_OBSERVATION_SECONDS", "14400"))
    if limit <= 0:
        raise SystemExit("OMEGA_EXECUTABLE_OBSERVATION_SECONDS must be positive")
    adapter = options.adapter.read_bytes()
    if len(adapter) != 2565 or \
            hashlib.sha256(adapter).hexdigest() != "ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38":
        raise SystemExit("execution adapter differs from the selected gate identity")
    subject = (directory / "epsilon_compiler.delta").read_bytes() + adapter
    support = (directory / "support.bin").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + support)
    receipt_limit = int(os.environ.get("OMEGA_EXECUTABLE_RECEIPT_SECONDS", "300"))
    if receipt_limit <= 0:
        raise SystemExit("OMEGA_EXECUTABLE_RECEIPT_SECONDS must be positive")
    receipt = evaluate(directory, (directory / "delta_compiler.gamma").read_bytes(),
                       request, receipt_limit, "Epsilon receipt reconstruction")
    if len(receipt) != 721484 or \
            hashlib.sha256(receipt).hexdigest() != "71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082":
        raise SystemExit("Epsilon execution receipt differs from the selected gate identity")
    # The controls matrix splits across five customers: one interpreted run
    # exhausts the evaluator's immutable pair arena (status 252 at the
    # 40,265,319th node) well before finishing all thirty-five invocations —
    # even an eighteen-invocation half does — so each part ends by
    # republishing the same successful tape and carries the same expected
    # observation.
    entry = gate / ("controls.epsilon" if options.controls else
                    "controls_b.epsilon" if options.controls_b else
                    "controls_c.epsilon" if options.controls_c else
                    "controls_d.epsilon" if options.controls_d else
                    "controls_e.epsilon" if options.controls_e else "main.epsilon")
    customer = (directory / "omega_compiler.epsilon").read_bytes() + entry.read_bytes()
    print(f"Compiler customer: {len(customer)} bytes, sha256 {hashlib.sha256(customer).hexdigest()}", flush=True)
    print(f"Omega source: {len(source)} bytes, sha256 {hashlib.sha256(source).hexdigest()}", flush=True)
    observation = evaluate(directory, receipt,
                           struct.pack("<I", len(customer)) + customer + source,
                           limit, "Omega source to Alpha tape")
    if controls:
        expected_tape = bytes.fromhex("13 0b00000000000000 00 00 01 00 2a00000000000000 14")
        if observation != b"\x00\x00\x00\x00\x00C" + expected_tape:
            raise SystemExit(f"Compiler controls failed: {observation[:40].hex()}")
        print("PASS: controls part folded operations, literal bounds, entry selection, refusals and reset")
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