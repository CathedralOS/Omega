#!/usr/bin/env python3
"""Exercise product request storage with a source-built bootstrap compiler.

An Epsilon evaluator runs the clear canary. A compiled construction executable
can instead check staging and reads using the complete product source unit.
"""
import argparse
from hashlib import sha256
from pathlib import Path
import runpy
import signal
import struct
from subprocess import TimeoutExpired
import sys
from tempfile import TemporaryDirectory
from time import monotonic

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/bootstrap"))
from compiled import execute, pack, stamp
from conformance.vectors import epsilon_request

CALLER = b"""machine main() -> u8 {
    let mut request: Request;
    request.length = 1;
    request.observed = 7 as +Trapping;
    request.bytes[65535] = 42;
    clear(@mut request);
    let canary: u8 = request.bytes[65535];
    let ok: bool = request.length == 0 && request.observed == 0 && remember(@mut request, canary);
    let result: u8 = request.bytes[0];
    result
}
machine remember(request: @mut Request, canary: u8) -> bool {
    request.bytes[0] = canary;
    true
}
"""

STAGE_CALLER = b"""machine main() -> u8 crashes Trap {
    let mut request: Request;
    clear(@mut request);
    request_stage(@mut request, 42);
    request_byte(@request, 0 as +Trapping)
}
"""

FULL_BUFFER_CALLER = b"""machine main() -> u8 crashes Trap {
    let mut request: Request;
    request.length = 65536;
    request.observed = 65536 as +Trapping;
    request.bytes[65535] = 42;
    request_stage(@mut request, 7);
    let last: u8 = request.bytes[65535];
    transition request.length == 65536 && request.observed == 65537 && last == 42 {
        true -> pass(),
        _ -> fail()
    }
    machine pass() -> u8 { 42 }
    machine fail() -> u8 { 7 }
}
"""


def staging_cases(product):
    encode = runpy.run_path(str(ROOT / "source/application/construction/construct.py"))["encode"]
    callers = (
        ("stage-first", STAGE_CALLER, 42),
        ("read-past-end", STAGE_CALLER.replace(b"0 as +Trapping", b"1 as +Trapping"), -signal.SIGILL),
        ("wrong-byte", STAGE_CALLER.replace(b"request, 42", b"request, true"), None),
        ("full-buffer", FULL_BUFFER_CALLER, 42),
    )
    for name, caller, expected in callers:
        packet, _, _, units = encode([("compiler_request", 0)], [], [(0, caller), (0, product)],
                                      0, b"main", paths=["main.omg", "request.omg"])
        if expected is None:
            expected = struct.pack("<BII", 0, 1, units[0][1] + caller.index(b"true"))
        yield name, packet, expected


def compile_request(executable: Path, request: bytes, timeout: int) -> bytes:
    started = monotonic()
    try:
        status, output = execute(executable, request, timeout=timeout)
    except TimeoutExpired as error:
        raise SystemExit(f"{executable} timed out after {timeout}s with "
                         f"{len(error.stdout or b'')} stdout bytes") from error
    if status != 0:
        raise SystemExit(f"{executable} exited {status} with {len(output)} stdout bytes, "
                         f"prefix {output[:40].hex()}")
    print(f"{len(output)} observation bytes sha256={sha256(output).hexdigest()} "
          f"in {monotonic() - started:.3f}s", flush=True)
    return output


def check_output(name, output, expected, work, timeout):
    if isinstance(expected, bytes):
        if output != expected:
            raise SystemExit(f"{name}: expected {expected.hex()}, got {output[:40].hex()}")
        coordinate = int.from_bytes(output[5:], "little")
        print(f"{name}: rejection at source byte {coordinate}, observation={output.hex()}")
        return
    if len(output) <= 5 or output[:5] != bytes(5):
        raise SystemExit(f"{name}: expected a successful compilation, got {output[:40].hex()}")
    tape = work / f"{name}.tape"
    payload = output[5:]
    tape.write_bytes(payload)
    program = work / name
    stamp(tape, program)
    actual = execute(program, timeout=timeout)
    if actual != (expected, b""):
        raise SystemExit(f"{name}: expected exit {expected} and empty stdout, got {actual!r}")
    outcome = signal.Signals(-expected).name if expected < 0 else f"exit {expected}"
    print(f"{name}: {len(payload)} Alpha bytes, {outcome}, sha256={sha256(payload).hexdigest()}", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    runner = parser.add_mutually_exclusive_group(required=True)
    runner.add_argument("evaluator", nargs="?", type=Path, help="Epsilon evaluator for the clear canary")
    runner.add_argument("--staging-constructor", type=Path, metavar="CONSTRUCTOR",
                        help="run the complete request unit through a compiled construction executable")
    parser.add_argument("--compare-with", type=Path, metavar="EXECUTABLE",
                        help="require a byte-identical observation from a second executable of the same kind")
    parser.add_argument("--timeout", type=int, default=600, metavar="SECONDS",
                        help="seconds per invocation (default 600)")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    product = (ROOT / "source/subsystems/compiler/request/request.omg").read_bytes()
    with TemporaryDirectory() as directory:
        work = Path(directory)
        if args.staging_constructor:
            executable = args.staging_constructor
            cases = staging_cases(product)
        else:
            executable = args.evaluator
            clear_start = product.index(b"public machine clear(")
            clear_end = product.index(b"\n}", clear_start) + 2
            program = product[:clear_end] + b"\n" + CALLER
            guest = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources", work / "omega.epsilon")
            guest += (ROOT / "tests/bootstrap/conformance/omega/main.epsilon").read_bytes()
            evaluator_source = pack("bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources", work / "epsilon.delta")
            request = epsilon_request(evaluator_source, guest, program)
            cases = [("Request.clear", request, 42)]

        executable = executable.resolve(strict=True)
        peer = args.compare_with.resolve(strict=True) if args.compare_with else None
        for binary in (executable, peer):
            if binary:
                print(f"{binary} sha256={sha256(binary.read_bytes()).hexdigest()}", flush=True)
        for name, request, expected in cases:
            print(f"{name}: {len(request)} request bytes sha256={sha256(request).hexdigest()}", flush=True)
            output = compile_request(executable, request, args.timeout)
            if peer and compile_request(peer, request, args.timeout) != output:
                raise SystemExit(f"{name}: the executables produced different observations")
            check_output(name, output, expected, work, args.timeout)


if __name__ == "__main__":
    main()
