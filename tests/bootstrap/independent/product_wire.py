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


def declarations(file, signatures):
    source = file.read_bytes()
    for signature in signatures:
        start = source.index(signature.encode())
        end = source.index(b"\n}", start) + 2
        yield source[start:end]


def byte_writer_input(encode):
    emitter = ROOT / "source/subsystems/compiler/alpha/emit.omg"
    cases = ROOT / "tests/bootstrap/independent/fixtures/alpha/byte_writer.omg"
    parts = declarations(emitter, ("public data AlphaEmitter ", "machine alpha_byte("))
    source = b"\n\n".join([*parts, cases.read_bytes()])
    return encode([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])[0]


def signed_mask_checker_input(encode):
    compiler = ROOT / "source/subsystems/compiler/source_to_psi"
    checker = compiler / "pipeline/typed_trees_to_checked_trees"
    files = [
        (compiler / "foundation/integers.omg", b"",
         ("public data ExactInteger ", "public machine exact_integer_negate(", "public machine exact_integer_at_most(")),
        (checker / "zone.omg", b"use integers.omg;",
         ("public data Bound ", "public machine bound_at_most(")),
        (checker / "evaluation.omg", b"use integers.omg;\nuse zone.omg;",
         ("public data Value ", "public machine value_unknown(", "public machine value_constant(",
          "public machine value_nonnegative(", "machine pick(", "public machine value_conjunction(")),
    ]
    cases = ROOT / "tests/bootstrap/independent/fixtures/interval/signed_mask.omg"
    sources = [(0, cases.read_bytes())]
    paths = ["main.omg"]
    for file, imports, signatures in files:
        sources.append((0, b"\n".join((imports, *declarations(file, signatures)))))
        paths.append(file.name)
    return encode([("app", 0)], [], sources, 0, b"main", paths=paths)[0]


def core_low_bytes_input(encode):
    core = ROOT / "source/library/core/numeric_conversion.omg"
    functions = [f"public machine narrow_u64_to_u{bits}_wrapping(" for bits in (8, 16, 32)]
    cases = ROOT / "tests/bootstrap/independent/fixtures/interval/low_bytes.omg"
    source = b"\n".join([*declarations(core, functions), cases.read_bytes()])
    return encode([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])[0]


def check_build_source_ownership(encode, runner, work, seconds):
    packages = [("app", 0), ("core", 1), ("library", 0)]
    entries = [(0, 0), (1, 1), (2, 1)]
    default = (ROOT / "source/library/core/defaults/build.omg").read_bytes()
    files = [(0, "build.omg", b"machine build(builder: @mutable BuildPlan) {}"), (1, "defaults/build.omg", default)]
    wire, _, _, _ = encode(packages, [], [(0, b"machine main() -> u8 {42}")], 0, b"main",
                            paths=["main.omg"], build_sources=files, build_entries=entries)
    # Mutate already encoded words to reach the Omega reader past the host's
    # own ownership checks. Offsets here count u32 words from the header.
    selections = 10 + 3 * len(packages) + 5
    sources = selections + 2 * len(entries)

    def replace_words(changes):
        changed = bytearray(wire)
        for word, value in changes.items():
            struct.pack_into("<I", changed, 4 * word, value)
        return changed

    first_path = struct.unpack_from("<2I", wire, 4 * (sources + 3))
    cases = {
        "build-replay-pending": (246, wire),
        "build-missing-unit": (253, replace_words({selections + 1: len(files)})),
        "build-duplicate-requester": (253, replace_words({selections + 2: 0})),
        "build-other-owner": (253, replace_words({selections + 5: 0})),
        "build-shadowed-default": (253, replace_words({selections + 1: 1})),
        "build-duplicate-file": (253, replace_words(
            {sources + 5: 0, sources + 8: first_path[0], sources + 9: first_path[1]})),
        "build-wrong-default-owner": (253, replace_words({sources + 5: 2})),
        "build-truncated-arena": (253, wire[:-1]),
    }
    for name, (status, request) in cases.items():
        (work / f"{name}.stdin").write_bytes(request)
        if execute(runner, request, seconds) != (status, b""):
            raise SystemExit(f"{name}: expected exit {status} with empty stdout")


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
    caller = b"use {wire}/codec.omg;\n" + word_cases.read_bytes().replace(b"u32_from_le_bytes(", b"|codec|::u32_from_le_bytes(")
    encode = runpy.run_path(str(ROOT / "source/application/construction/construct.py"))["encode"]
    packages = [("app", 0), ("wire", 0)]
    units = [(0, caller), (1, codec)]
    linked, _, _, _ = encode(packages, [(0, 1, "wire")], units, 0, b"main", paths=["main.omg", "codec.omg"])
    unlinked, _, _, unit_rows = encode(packages, [], units, 0, b"main", paths=["main.omg", "codec.omg"])

    inputs = {"linked": linked, "missing-edge": unlinked, "trailing-byte": linked + b"\0"}
    if args.constructor or args.evaluator:
        inputs["alpha-byte-writer"] = byte_writer_input(encode)
        inputs["signed-mask-fact"] = signed_mask_checker_input(encode)
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
        check_build_source_ownership(encode, runner, work, args.seconds)
        mask_request = core_low_bytes_input(encode)
        (work / "core-low-bytes.stdin").write_bytes(mask_request)
        status, tape = execute(runner, mask_request, args.seconds)
        if status:
            raise SystemExit(f"Omega constructor refused core low-byte conversions with exit {status}")
        check_tape(work, tape, args.seconds, "core-low-bytes")
        too_wide = (b"machine convert(value:u64)->u8 {(value & 0x1FF) as u8} "
                    b"machine main()->u8 {convert(0x100)}")
        wide_request = encode([("app", 0)], [], [(0, too_wide)], 0, b"main", paths=["main.omg"])[0]
        (work / "nine-bit-byte-cast.stdin").write_bytes(wide_request)
        if execute(runner, wide_request, args.seconds) != (247, b""):
            raise SystemExit("nine-bit mask must fail the u8 fit check before publication")
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
            request, _, _, _ = encode(packages, [(0, 1, "wire")], [(0, source), (1, codec)], 0, b"main", paths=["main.omg", "codec.omg"])
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
            request, _, _, _ = encode(packages, [(0, 1, "wire")], [(0, source), (1, codec)], 0, b"main", paths=["main.omg", "codec.omg"])
            (work / f"{name}.stdin").write_bytes(request)
            if execute(runner, request, args.seconds) != (expected, b""):
                raise SystemExit(f"{name}: expected exit {expected} with empty stdout")
        service_dir = ROOT / "tests/bootstrap/independent/fixtures/service"
        service_entry = (service_dir / "caller.omg").read_bytes()
        marker = (service_dir / "ground.omg").read_bytes()
        service_packages = packages + [("service", 0)]
        edges = [(0, 1, "wire"), (0, 2, "service")]
        for name, entry_source, service_source, expected in (
            ("marker", service_entry, marker, 0),
            ("excess-service", service_entry.replace(b"main() reaches |ground|::Input", b"main() reaches |ground|::Other"), marker, 247),
            ("empty-service", service_entry.replace(b"main() reaches |ground|::Input", b"main() reaches none"), marker, 247),
            ("uninstalled-operation",
             service_entry.replace(b"_ = |codec|::u32_from_le_bytes(42, 0, 0, 0);", b"_ = |ground|::Input::read();"),
             marker.replace(b"Input {}", b"Input { machine read() -> u8 reaches Input crashes none; }"), 246),
            ("unresolved-service", service_entry.replace(b"reaches |ground|::Input", b"reaches Missing"), marker, 248),
            ("ordinary-trait", service_entry, marker.replace(b"boundary trait", b"trait"), 248),
            ("unapplied-service", service_entry, marker.replace(b"Input {}", b"Input<T> {}"), 248),
            ("bare-reaches", service_entry.replace(b"reaches |ground|::Input", b"reaches"), marker, 250),
        ):
            units = [(0, entry_source), (1, codec), (2, service_source)]
            request, _, _, _ = encode(service_packages, edges, units, 0, b"main", paths=["main.omg", "codec.omg", "ground.omg"])
            (work / f"{name}.stdin").write_bytes(request)
            status, tape = execute(runner, request, args.seconds)
            if status != expected or (status and tape):
                raise SystemExit(f"{name}: expected compiler exit {expected}, got {status} with {len(tape)} bytes")
            if not status:
                check_tape(work, tape, args.seconds, name, 0)
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

    for name in ("signed-mask-fact", "alpha-byte-writer"):
        output = evaluate(name)
        if output[:5] != bytes(5) or len(output) <= 5:
            raise SystemExit(f"{name} compilation failed: {output[:40].hex()}")
        check_tape(work, output[5:], args.seconds, name)

    # The first source unit starts with the unresolved import. A compiler
    # refusal publishes its coordinate; malformed transport publishes none.
    caller_start = unit_rows[0][1]
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
