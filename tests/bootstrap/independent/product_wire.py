#!/usr/bin/env python3
"""Run the product wire package through the bootstrap compiler's construction entry."""
import argparse
from hashlib import sha256
from pathlib import Path
import runpy
import struct
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/bootstrap"))
from compiled import execute, pack, stamp
from conformance.vectors import epsilon_request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="new directory for the construction inputs")
    route = parser.add_mutually_exclusive_group()
    route.add_argument("--evaluator", type=Path, help="also run and check a source-built Epsilon evaluator")
    route.add_argument("--constructor", type=Path, help="run and check a compiled construction executable")
    parser.add_argument("--seconds", type=int, default=600, help="timeout per execution")
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
    tape = work / "wire.tape"
    tape.write_bytes(linked_output[5:])
    stamp(tape, work / "wire")
    if execute(work / "wire", timeout=args.seconds) != (42, b""):
        raise SystemExit("wire program did not exit 42 with empty stdout")
    print(f"Wire exited 42. Alpha tape has {len(linked_output) - 5} bytes, "
          f"sha256={sha256(linked_output[5:]).hexdigest()}", flush=True)

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
