"""Compile ordinary Omega source through the selected Epsilon implementation."""

# Every customer entry the harness may select is bound: the packed D closure
# plus exactly one of these gate-local files forms the evaluated customer.
ENTRY_IDENTITIES = {
    "main.epsilon": (1757,
                     "c0af3126f13c8c511d04f224e630f60f3316c0f9fa6e310e72f66779c7c3ce9e"),
    "main_ocreq.epsilon": (19249,
                           "5d5d0b8ed0146b055ffdbb6d680bb902a0e350c80b13577bf148879c6c753943"),
    "controls.epsilon": (3631,
                         "78995d1f7975bbd7b8d82230b557f43bb263addb5be3deb2b9bf56cb03efa0a9"),
    "controls_b.epsilon": (3084,
                           "261d1529b50ab7b36c9dd228a0df7a4250d46d2913dcd85897ee8b911e98dbc3"),
    "controls_c.epsilon": (2824,
                           "0dbc7da705e7da63a7589b49a25677037dcd43c31c3266f31511986b3eba54ae"),
    "controls_d.epsilon": (2850,
                           "916218b57476fe59f22a2d493b6529502e3ac3a4fda856d4e16b9a156a0f57c9"),
    "controls_e.epsilon": (2797,
                           "83ce536dacd5efb9238d7f5869ed5d6269c6481a24b4cdd0b7d3985777c150bc"),
    "controls_f.epsilon": (4425,
                           "fbc7ed2868f9e70833fdfc36c927238c8fd11184e5127e372d732c8ab6ebff0e"),
    "controls_g.epsilon": (3127,
                           "3c94d2e5430226dbeb20b311d5336f57ab11c8fd49ace137e44785fcbac6ecb9"),
    "controls_h.epsilon": (3193,
                           "b48c672f09c8263d9d352fdb37af66a82c3083df38dabd533a93e0573e9e5c0e"),
    "controls_i.epsilon": (3863,
                           "8f583b6510c940e3ef0cdc0223a1e263da37ea4f6decbea1a3130f4ba33645d0"),
    "controls_j.epsilon": (4461,
                           "406e2bc4353983ebc81d6daa215a9b42f72b85056bc5f3263e1917e7e01efb94"),
}

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


def require_identity(label, source, length, digest):
    actual = hashlib.sha256(source).hexdigest()
    if len(source) != length or actual != digest:
        raise SystemExit(
            f"{label} identity changed: {len(source)} bytes, SHA-256 {actual}")


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
    limit = int(os.environ.get("OMEGA_EXECUTABLE_OBSERVATION_SECONDS", "14400"))
    if limit <= 0:
        raise SystemExit("OMEGA_EXECUTABLE_OBSERVATION_SECONDS must be positive")
    adapter = options.adapter.read_bytes()
    require_identity("execution adapter", adapter, 2565,
                     "ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38")
    epsilon = (directory / "epsilon_compiler.delta").read_bytes()
    require_identity("Epsilon", epsilon, 617354,
                     "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e")
    delta_compiler = (directory / "delta_compiler.gamma").read_bytes()
    require_identity("Delta compiler", delta_compiler, 147840,
                     "fbcb9e17b7ce0c75849136086bc5a4b6df4264054be72b5aae6d70325f9d0929")
    compiler = (directory / "omega_compiler.epsilon").read_bytes()
    require_identity("D", compiler, 569920,
                     "f5f051fba1ac62322cc1b0af9f3dc8e5fb1951feef24e44a627f1d9e4c28f842")
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
        receipt = evaluate(directory, delta_compiler,
                           request, receipt_limit, "Epsilon receipt reconstruction")
    require_identity("Epsilon execution receipt", receipt, 721484,
                     "71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082")
    if receipt_cache and not Path(receipt_cache).exists():
        Path(receipt_cache).write_bytes(receipt)
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