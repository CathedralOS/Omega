#!/usr/bin/env python3
"""Run the product's Request and clear through a constructed Epsilon evaluator.

Pass a Linux evaluator executable, optionally with a second evaluator to compare.
Both receive the same request. The shared helpers pack checked-in sources and
run the emitted Alpha tape.
"""
import argparse
from hashlib import sha256
from pathlib import Path
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


def compile_request(evaluator: Path, request: bytes, timeout: int) -> bytes:
    executable = evaluator.resolve(strict=True)
    print(f"{executable} sha256={sha256(executable.read_bytes()).hexdigest()}", flush=True)
    started = monotonic()
    try:
        status, output = execute(executable, request, timeout=timeout)
    except TimeoutExpired as error:
        raise SystemExit(f"{executable} timed out after {timeout}s with "
                         f"{len(error.stdout or b'')} stdout bytes") from error
    if status != 0 or len(output) <= 5 or output[:5] != bytes(5):
        raise SystemExit(f"{executable} exited {status} with {len(output)} stdout bytes, "
                         f"prefix {output[:40].hex()}")
    print(f"{len(output)} observation bytes sha256={sha256(output).hexdigest()} "
          f"in {monotonic() - started:.3f}s", flush=True)
    return output


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evaluator", type=Path)
    parser.add_argument("--compare-with", type=Path, metavar="EVALUATOR",
                        help="require a byte-identical observation from a second evaluator")
    parser.add_argument("--timeout", type=int, default=600, metavar="SECONDS",
                        help="seconds per evaluator invocation (default 600)")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    product = (ROOT / "source/subsystems/compiler/request/request.omg").read_bytes()
    # Stop after the complete clear declaration. Staging and decoding have
    # their own product gate and require a larger bootstrap language subset.
    clear_start = product.index(b"public machine clear(")
    clear_end = product.index(b"\n}", clear_start) + 2
    program = product[:clear_end] + b"\n" + CALLER

    with TemporaryDirectory() as directory:
        work = Path(directory)
        guest = pack("bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources", work / "omega.epsilon")
        guest += (ROOT / "tests/bootstrap/conformance/omega/main.epsilon").read_bytes()
        evaluator_source = pack("bootstrap/interpreted/4_epsilon/epsilon_compiler.delta.sources", work / "epsilon.delta")
        request = epsilon_request(evaluator_source, guest, program)
        print(f"{len(request)} request bytes sha256={sha256(request).hexdigest()}", flush=True)
        output = compile_request(args.evaluator, request, args.timeout)
        if args.compare_with:
            peer_output = compile_request(args.compare_with, request, args.timeout)
            if peer_output != output:
                raise SystemExit("the evaluators produced different observations")
        tape = work / "request.tape"
        tape.write_bytes(output[5:])
        executable = work / "request"
        stamp(tape, executable)
        result = execute(executable)
        if result != (42, b""):
            raise SystemExit(f"expected exit 42 and empty stdout, got {result!r}")
        print(f"Request.clear: {tape.stat().st_size} Alpha bytes, exit 42")


if __name__ == "__main__":
    main()
