#!/usr/bin/env python3
"""Run the product wire package through the bootstrap compiler's construction entry."""
import argparse
from hashlib import sha256
from pathlib import Path
import runpy
import signal
import struct
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/bootstrap"))
from compiled import add_invocation_budget, execute, pack, stamp
from conformance.vectors import epsilon_request


def check_tape(work, data, seconds, name="wire", expected=42):
    if not data:
        raise SystemExit(f"{name} compilation produced no tape")
    tape = work / f"{name}.tape"
    tape.write_bytes(data)
    stamp(tape, work / name)
    if execute(work / name, timeout=seconds) != (expected, b""):
        raise SystemExit(f"{name} did not exit {expected} with empty stdout")
    print(f"{name} exited {expected}. Alpha tape has {len(data)} bytes, sha256={sha256(data).hexdigest()}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new directory for the construction inputs")
    route = parser.add_mutually_exclusive_group()
    route.add_argument("--evaluator", type=Path, help="also run and check a source-built Epsilon evaluator")
    route.add_argument("--constructor", type=Path, help="run and check a compiled construction executable")
    route.add_argument("--omega-constructor", type=Path, help="check the Omega-written construction executable")
    add_invocation_budget(parser)
    args = parser.parse_args()
    work = args.output.resolve()
    work.mkdir(parents=True)

    codec = (ROOT / "source/foundation/wire/codec.omg").read_bytes()
    word_cases = ROOT / "tests/bootstrap/conformance/omega/wire_word.omg"
    caller = b"use wire\\u32_from_le_bytes;\n" + word_cases.read_bytes()
    encode = runpy.run_path(str(ROOT / "source/application/construction/construct.py"))["encode"]
    packages = [("app", 0), ("wire", 0)]
    units = [(0, caller), (1, codec)]
    linked, _, _, _ = encode(packages, [(0, 1, "wire")], units, 0, b"main")
    unlinked, _, _, unit_rows = encode(packages, [], units, 0, b"main")

    inputs = {"linked": linked, "missing-edge": unlinked, "trailing-byte": linked + b"\0"}
    # Each truncated packet keeps the declared arena length. Sweep through
    # the partial final group and a preceding full group in the native entry.
    truncations = {}
    if args.constructor:
        truncations = {f"missing-{count}-bytes": linked[:-count] for count in range(1, 17)}
        inputs.update(truncations)
    for name, stdin in inputs.items():
        (work / f"{name}.stdin").write_bytes(stdin)
    if args.omega_constructor:
        runner = args.omega_constructor.resolve(strict=True)
        status, output = execute(runner, linked, args.seconds)
        if status:
            raise SystemExit(f"Omega constructor refused linked wire package with exit {status}")
        check_tape(work, output, args.seconds)
        # Run the same imported decoder from a resultless application entry.
        # One changed input must take its checked Trap branch through Alpha.
        case = ROOT / "tests/bootstrap/independent/fixtures/wire/resultless.omg"
        unit_caller = case.read_bytes()
        trapped = unit_caller.replace(b"bytes(42", b"bytes(43")
        for name, source, expected in (
            ("unit", unit_caller, 0),
            ("trap", trapped, -signal.SIGILL),
            ("quiet", b"machine main() crashes none {}", 0),
        ):
            request, _, _, _ = encode(packages, [(0, 1, "wire")], [(0, source), (1, codec)], 0, b"main")
            (work / f"{name}.stdin").write_bytes(request)
            status, tape = execute(runner, request, args.seconds)
            if status:
                raise SystemExit(f"Omega constructor refused {name} with exit {status}")
            check_tape(work, tape, args.seconds, name, expected)
        for name, source, expected in (
            ("forbidden-trap", trapped.replace(b"failed() crashes Trap", b"failed() crashes none"), 247),
            ("unsupported-abort", trapped.replace(b"Trap", b"Abort"), 246),
            ("unsupported-result", b"machine main() -> u32 { 42 }", 246),
            ("mixed-none-first", unit_caller.replace(b"main() crashes Trap", b"main() crashes none crashes Trap"), 250),
            ("mixed-none-last", unit_caller.replace(b"main() crashes Trap", b"main() crashes Trap crashes none"), 250),
        ):
            request, _, _, _ = encode(packages, [(0, 1, "wire")], [(0, source), (1, codec)], 0, b"main")
            (work / f"{name}.stdin").write_bytes(request)
            if execute(runner, request, args.seconds) != (expected, b""):
                raise SystemExit(f"{name}: expected exit {expected} with empty stdout")
        for name, expected in (("missing-edge", 248), ("trailing-byte", 253)):
            if execute(runner, inputs[name], args.seconds) != (expected, b""):
                raise SystemExit(f"{name}: expected exit {expected} with empty stdout")
        return
    if args.evaluator:
        runner = args.evaluator.resolve(strict=True)
        closure = pack("bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources", work / "evaluator.delta")
        compiler = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources", work / "omega.epsilon")
        guest = compiler + (ROOT / "bootstrap/omega/construct_omega.epsilon").read_bytes()
        requests = {name: epsilon_request(closure, guest, stdin) for name, stdin in inputs.items()}
        for name, request in requests.items():
            (work / f"{name}.eereq").write_bytes(request)
    elif args.constructor:
        runner = args.constructor.resolve(strict=True)
        requests = inputs
    else:
        return

    def evaluate(name):
        request = requests[name]
        print(f"Running {name} ({len(request)} request bytes)", flush=True)
        status, output = execute(runner, request, args.seconds)
        (work / f"{name}.eeout").write_bytes(output)
        if status:
            raise SystemExit(f"{name}: {runner.name} exited {status}")
        return output

    linked_output = evaluate("linked")
    if linked_output[:5] != bytes(5) or len(linked_output) <= 5:
        raise SystemExit(f"wire compilation failed: {linked_output[:40].hex()}")
    check_tape(work, linked_output[5:], args.seconds)

    # The first source unit starts with the unresolved import. A compiler
    # refusal publishes its coordinate; malformed transport publishes none.
    _, caller_start, _ = unit_rows[0]
    refusal = evaluate("missing-edge")
    if refusal != b"\x00" + struct.pack("<II", 1, caller_start):
        raise SystemExit(f"missing edge: expected refusal at {caller_start}, got {refusal[:40].hex()}")
    malformed = evaluate("trailing-byte")
    if malformed != b"\x00" + struct.pack("<I", 3):
        raise SystemExit(f"trailing byte: expected framing failure, got {malformed[:40].hex()}")
    print(f"Missing edge refused at byte {caller_start}. Trailing byte refused before compilation.", flush=True)
    for name in truncations:
        if evaluate(name) != b"\x00" + struct.pack("<I", 3):
            raise SystemExit(f"{name}: truncated input must exit 3 without a diagnostic")


if __name__ == "__main__":
    main()
