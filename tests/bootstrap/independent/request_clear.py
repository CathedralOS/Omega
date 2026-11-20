#!/usr/bin/env python3
"""Run the product's Request and clear through a constructed Epsilon evaluator.

Pass the Linux evaluator executable as the sole argument. The shared bootstrap
helpers pack the checked-in compiler sources and run the emitted Alpha tape.
"""
import argparse
from pathlib import Path
import sys
from tempfile import TemporaryDirectory

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


def main() -> None:
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("evaluator", type=Path)
    evaluator = args.parse_args().evaluator.resolve()
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
        status, output = execute(evaluator, epsilon_request(evaluator_source, guest, program), timeout=600)
        assert status == 0 and output[:5] == bytes(5) and len(output) > 5, (status, output[:40])
        tape = work / "request.tape"
        tape.write_bytes(output[5:])
        executable = work / "request"
        stamp(tape, executable)
        assert execute(executable) == (42, b"")
        print(f"Request.clear: {tape.stat().st_size} Alpha bytes, exit 42")


if __name__ == "__main__":
    main()
