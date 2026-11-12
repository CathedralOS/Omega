"""Compile ordinary Omega source through the selected Epsilon implementation."""

# Every customer entry the harness may select is bound: the packed D closure
# plus exactly one of these gate-local files forms the evaluated customer.
ENTRY_IDENTITIES = {
    "main.epsilon": (1757,
                     "c0af3126f13c8c511d04f224e630f60f3316c0f9fa6e310e72f66779c7c3ce9e"),
    "main_ocreq.epsilon": (19249,
                           "5d5d0b8ed0146b055ffdbb6d680bb902a0e350c80b13577bf148879c6c753943"),
    "controls.epsilon": (3539,
                           "16a4915fedac6e95a50cb2b9405c988d995935146d0552606b698d3f52be8740"),
    "controls_b.epsilon": (2804,
                           "28701862d665951b2e432c58c77b7b1bcbc9d2b8ae53417f95bc76fa19908c6e"),
    "controls_c.epsilon": (2834,
                           "1ec2b16b0d97631bc88f23c0d39c417e6184875c1178913fba4629b815fc3465"),
    "controls_d.epsilon": (2852,
                           "b54adbcd30b9f192217b89b96efd3b548443a6e9426bb7891c98845fa89e8adb"),
    "controls_e.epsilon": (2795,
                           "881feab7746fe17185dfa163629b13c788274891f43c9ab7d72d1f07a3272d72"),
    "controls_f.epsilon": (4465,
                           "a144d2beb326b1b4f6d0251c97089c5b54dae5d3fc737d6b573a7e2ef7888190"),
    "controls_g.epsilon": (3175,
                           "c3fb524c98a52ad999832af0b7525e75f98900ec69b6f6ac965348a3a20da137"),
    "controls_h.epsilon": (3249,
                           "30a4006338f1eb08a169fb6b2d844881adb3314b9c39c51fcaea6029838964bc"),
    "controls_i.epsilon": (3754,
                           "23139dabc77402e41850ff841c1961fba6d0c3713cf30392634186d0159c678f"),
    "controls_j.epsilon": (4571,
                           "c0226b7ac5ffab6e91fc8edfcc3f64fa3e4e179b20489b77722e8db6d81e96cd"),
}

import argparse
import hashlib
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from gate_lib import (env_seconds, evaluate, require_identity,  # noqa: E402
                    require_receipt_identity, run)


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
    arguments.add_argument("--controls-j", action="store_true")
    arguments.add_argument("--identity", action="store_true")
    options = arguments.parse_args()
    directory = options.directory.resolve()
    gate = Path(__file__).resolve().parent
    selected = [options.controls, options.controls_b, options.controls_c,
                options.controls_d, options.controls_e, options.controls_f,
                options.controls_g, options.controls_h, options.controls_i,
                options.controls_j]
    if sum(1 for flag in selected if flag) > 1:
        arguments.error("controls parts are separate runs")
    if any(selected) and \
            (options.source or options.expect is not None or options.expect_compile):
        arguments.error("controls cannot be combined with source or expected-outcome options")
    expected_exit = options.expect if options.expect is not None else int((gate / "expected.txt").read_text())
    controls = any(selected)
    if options.ocreq and options.diagnostic:
        arguments.error("--ocreq and --diagnostic select different entries")
    if options.ocreq and controls:
        arguments.error("the OCREQ request route does not drive the diagnostic controls")
    if options.diagnostic and controls:
        arguments.error("the diagnostic adapter does not drive the controls")
    source = b"" if controls else (options.source or gate / "program.omg").read_bytes()
    limit = env_seconds("OMEGA_EXECUTABLE_OBSERVATION_SECONDS", "14400")
    adapter = options.adapter.read_bytes()
    require_identity("execution adapter", adapter, 2565,
                     "ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38")
    epsilon = (directory / "epsilon_compiler.delta").read_bytes()
    require_identity("Epsilon", epsilon, 617354,
                     "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e")
    delta_compiler = (directory / "delta_compiler.gamma").read_bytes()
    require_identity("Delta compiler", delta_compiler, 151525,
                     "de94946dd946c90c3311dba52aa4622a3c80a18866d5483531de1312cc62e0c6")
    compiler = (directory / "omega_compiler.epsilon").read_bytes()
    require_identity("D", compiler, 714400,
                     "30e6ea13e8ccab73633af4887dfb3d5ef00d1c25a114f01496aa2290cd6c9511")
    subject = epsilon + adapter
    support = (directory / "support.bin").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + support)
    # The controls matrix splits across ten customers, so each part ends by
    # republishing the same successful tape and carries the same expected
    # observation. The split predates the V5 arena growth: the retired
    # 40,265,318-node pair arena could not retain even an eighteen-invocation
    # half, and the ten-way shape remains the bounded evaluated form.
    entry_name = ("controls.epsilon" if options.controls else
                  "controls_b.epsilon" if options.controls_b else
                  "controls_c.epsilon" if options.controls_c else
                  "controls_d.epsilon" if options.controls_d else
                  "controls_e.epsilon" if options.controls_e else
                  "controls_f.epsilon" if options.controls_f else
                  "controls_g.epsilon" if options.controls_g else
                  "controls_h.epsilon" if options.controls_h else
                  "controls_i.epsilon" if options.controls_i else
                  "controls_j.epsilon" if options.controls_j else
                  "main.epsilon" if options.diagnostic else
                  "main_ocreq.epsilon")
    entry = (gate / entry_name).read_bytes()
    entry_size, entry_sha256 = ENTRY_IDENTITIES[entry_name]
    require_identity(f"{entry_name} customer entry", entry,
                     entry_size, entry_sha256)
    customer = compiler + entry
    print(f"Compiler customer: {len(customer)} bytes, sha256 {hashlib.sha256(customer).hexdigest()}", flush=True)
    print(f"Omega source: {len(source)} bytes, sha256 {hashlib.sha256(source).hexdigest()}", flush=True)
    # The gate's entry is the real compiler request route: an OCREQ V1 frame
    # whose subject carries one application package snapshotting the source
    # file, and whose invocation selects the alpha_bootstrap_tape product on
    # the alpha_bootstrap profile with the SHA-256 subject commitment bound.
    # --ocreq is the explicit spelling of that default route; --diagnostic
    # keeps the raw-source adapter lane for refusal coverage.
    request_route = options.ocreq or not (controls or options.diagnostic)
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
    if options.identity:
        # Host-free leg: every bound identity above is checked and the request,
        # customer, and sealed-input byte streams are fully assembled; only the
        # evaluator executions need a seed host.
        (directory / "evaluator.exe").stat()
        print(f"Omega executable: identity legs green; execution legs need a "
              f"seed host ({len(request)}-byte receipt request, "
              f"{len(customer)}-byte customer, {len(sealed_input)}-byte sealed "
              f"input)", flush=True)
        return
    receipt_limit = env_seconds("OMEGA_EXECUTABLE_RECEIPT_SECONDS", "1800")
    # The receipt depends only on the pinned delta compiler and request, so the
    # controls matrix may share one reconstruction through an optional cache;
    # the pinned identity check below still applies to cached bytes.
    receipt_cache = os.environ.get("OMEGA_EXECUTABLE_RECEIPT_CACHE")
    receipt = None
    if receipt_cache and Path(receipt_cache).exists():
        receipt = Path(receipt_cache).read_bytes()
        print("Epsilon receipt reconstruction: cache hit", flush=True)
    if receipt is None:
        receipt = evaluate(directory, delta_compiler,
                           request, receipt_limit, "Epsilon receipt reconstruction",
                           "no compiler judgment")
    require_receipt_identity(receipt)
    if receipt_cache and not Path(receipt_cache).exists():
        Path(receipt_cache).write_bytes(receipt)
    observation = evaluate(directory, receipt,
                           struct.pack("<I", len(customer)) + customer + sealed_input,
                           limit, "Omega source to Alpha tape", "no compiler judgment")
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
                               "Emitted Alpha program",
                               "Alpha execution did not finish")
    if code != expected_exit or output or errors:
        raise SystemExit(f"Program result {code}, stdout {output!r}, stderr {errors!r}; expected {expected_exit}")
    print(f"PASS: {options.source or gate / 'program.omg'} -> {directory / 'program.tape'} -> exit {code}")


if __name__ == "__main__":
    main()