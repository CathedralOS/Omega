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


def ocreq_request(source, path):
    """Frame the canonical OCREQ V1 request for this gate's single-source
    program: one application package whose snapshot is exactly `path` as a
    non-executable regular file, an empty edge table, and an invocation
    selecting the alpha_bootstrap_tape product on the alpha_bootstrap
    target profile with the bound SHA-256 subject commitment."""
    def u32(value):
        return struct.pack("<I", value)

    def field(payload):
        return u32(len(payload)) + payload

    snapshot_row = u32(2) + field(path) + u32(0) + field(source)
    package_row = (
        field(b"program")        # name
        + u32(1)                 # lineage_kind: external_local
        + field(b"")             # lineage
        + field(b"")             # revision
        + field(b"")             # tree
        + field(b"")             # content
        + field(b"")             # member projection
        + u32(2)                 # role: application
        + u32(1) + snapshot_row  # snapshot table
    )
    subject = u32(1) + package_row + u32(0) + u32(0) + u32(2)
    commitment = hashlib.sha256(
        b"omega.ocreq.subject.sha256.v1\x00" + subject).digest()
    invocation = u32(4) + field(b"alpha_bootstrap") + u32(0) + commitment
    return (b"OCREQ\x01\x00\x00" + u32(len(subject)) + u32(len(invocation))
            + subject + invocation)


def main():
    arguments = argparse.ArgumentParser()
    arguments.add_argument("directory", type=Path)
    arguments.add_argument("adapter", type=Path)
    arguments.add_argument("--source", type=Path)
    arguments.add_argument("--expect", type=int)
    arguments.add_argument("--expect-compile", type=int, choices=range(5), default=0)
    arguments.add_argument("--ocreq", action="store_true")
    arguments.add_argument("--diagnostic", action="store_true")
    arguments.add_argument("--controls", action="store_true")
    arguments.add_argument("--controls-b", action="store_true")
    arguments.add_argument("--controls-c", action="store_true")
    arguments.add_argument("--controls-d", action="store_true")
    arguments.add_argument("--controls-e", action="store_true")
    arguments.add_argument("--controls-f", action="store_true")
    arguments.add_argument("--controls-g", action="store_true")
    arguments.add_argument("--controls-h", action="store_true")
    arguments.add_argument("--controls-i", action="store_true")
    options = arguments.parse_args()
    directory = options.directory.resolve()
    gate = Path(__file__).resolve().parent
    selected = [options.controls, options.controls_b, options.controls_c,
                options.controls_d, options.controls_e, options.controls_f,
                options.controls_g, options.controls_h, options.controls_i]
    if sum(1 for flag in selected if flag) > 1:
        arguments.error("controls parts are separate runs")
    if any(selected) and \
            (options.source or options.expect is not None or options.expect_compile):
        arguments.error("controls cannot be combined with source or expected-outcome options")
    expected_exit = options.expect if options.expect is not None else int((gate / "expected.txt").read_text())
    controls = any(selected)
    if options.ocreq and options.diagnostic:
        arguments.error("--ocreq and --diagnostic select different entries")
    if options.diagnostic and controls:
        arguments.error("the diagnostic adapter does not drive the controls")
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
    # The receipt depends only on the pinned delta compiler and request, so the
    # controls matrix may share one reconstruction through an optional cache;
    # the pinned identity check below still applies to cached bytes.
    receipt_cache = os.environ.get("OMEGA_EXECUTABLE_RECEIPT_CACHE")
    receipt = None
    if receipt_cache and Path(receipt_cache).exists():
        receipt = Path(receipt_cache).read_bytes()
        print("Epsilon receipt reconstruction: cache hit", flush=True)
    if receipt is None:
        receipt = evaluate(directory, (directory / "delta_compiler.gamma").read_bytes(),
                           request, receipt_limit, "Epsilon receipt reconstruction")
    if len(receipt) != 721484 or \
            hashlib.sha256(receipt).hexdigest() != "71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082":
        raise SystemExit("Epsilon execution receipt differs from the selected gate identity")
    if receipt_cache and not Path(receipt_cache).exists():
        Path(receipt_cache).write_bytes(receipt)
    # The controls matrix splits across nine customers, so each part ends by
    # republishing the same successful tape and carries the same expected
    # observation. The split predates the V5 arena growth: the retired
    # 40,265,318-node pair arena could not retain even an eighteen-invocation
    # half, and the nine-way shape remains the bounded evaluated form.
    entry = gate / ("controls.epsilon" if options.controls else
                    "controls_b.epsilon" if options.controls_b else
                    "controls_c.epsilon" if options.controls_c else
                    "controls_d.epsilon" if options.controls_d else
                    "controls_e.epsilon" if options.controls_e else
                    "controls_f.epsilon" if options.controls_f else
                    "controls_g.epsilon" if options.controls_g else
                    "controls_h.epsilon" if options.controls_h else
                    "controls_i.epsilon" if options.controls_i else
                    "main.epsilon" if options.diagnostic else
                    "main_ocreq.epsilon")
    customer = (directory / "omega_compiler.epsilon").read_bytes() + entry.read_bytes()
    print(f"Compiler customer: {len(customer)} bytes, sha256 {hashlib.sha256(customer).hexdigest()}", flush=True)
    print(f"Omega source: {len(source)} bytes, sha256 {hashlib.sha256(source).hexdigest()}", flush=True)
    # The gate's entry is the real compiler request route: an OCREQ V1 frame
    # whose subject carries one application package snapshotting the source
    # file, and whose invocation selects the alpha_bootstrap_tape product on
    # the alpha_bootstrap profile with the SHA-256 subject commitment bound.
    # --diagnostic keeps the raw-source adapter lane for refusal coverage.
    request_route = not (controls or options.diagnostic)
    if request_route:
        snapshot_path = Path(options.source).name.encode() if options.source \
            else b"program.omg"
        sealed_input = ocreq_request(source, snapshot_path)
        print(f"OCREQ request: {len(sealed_input)} bytes, "
              f"sha256 {hashlib.sha256(sealed_input).hexdigest()}", flush=True)
    else:
        sealed_input = source
        print(f"Omega source: {len(sealed_input)} bytes, "
              f"sha256 {hashlib.sha256(sealed_input).hexdigest()}", flush=True)
    observation = evaluate(directory, receipt,
                           struct.pack("<I", len(customer)) + customer + sealed_input,
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
        refusal = observation[5:]
        if request_route:
            # The request route publishes exactly one closed OCOUT V1 frame
            # on every refusal: identity, matching outcome tag, and the
            # 40/48-byte extents the contract defines.
            if len(refusal) not in (40, 48) or \
                    refusal[:8] != b"\xffOCOUT\x01\x00" or \
                    refusal[8] != options.expect_compile:
                raise SystemExit(
                    f"Refusal did not publish its OCOUT frame: {refusal[:40].hex()}")
            print("Expected compiler refusal with its OCOUT frame", flush=True)
            return
        if refusal:
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