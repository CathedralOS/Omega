#!/usr/bin/env python3
"""Construct the Gamma compiler through Beta/Gamma, rebuild, and run customers.

Python frames bytes and compares observations. Only --accelerated invokes the
host Gamma compiler as a semantic producer; its results are labeled separately.
"""

import argparse
import hashlib
from pathlib import Path
import time

import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compiled
import packed_tape


def digest(data):
    return hashlib.sha256(data).hexdigest()


def large_tape_control(work, compiler, seconds):
    # The straight Alpha encoding of the dead definitions alone exceeds
    # the seed hole. A bad call in the very last one must still be checked.
    body = b"(/ 42 1)"
    for _ in range(7):
        body = b"(+ " + body + b" " + body + b")"
    prefix = b"""(def $application () Int 1)
(def main () Int (later (read 0)))
"""
    padding = b"".join(b"(def pad" + str(n).encode() + b" () Int " + body + b")\n"
                       for n in range(700))
    guest = prefix + padding + b"""(def later ((which Int)) Int
  (if (eq which 0)
    (let tag Int (write (second (pair -9223372036854775808 (read 1)))) (pair 42 1))
    (if (eq which 1) (pair 17 0) (pair (/ -9223372036854775808 -1) 1))))
"""
    tape = work / "large.tape"
    status, program = compiled.execute(compiler, guest, seconds)
    assert status == 0 and 0 < len(program) <= 16_777_212
    expanded, _, _ = packed_tape.inspect(program)
    assert expanded > 16_777_212, "large control fits in the ordinary stamp"
    tape.write_bytes(program)
    executable = work / "large"
    compiled.stamp(tape, executable)
    for stdin, expected in ((b"\0Z", (42, b"Z")), (b"\1", (17, b"")), (b"\2", (249, b""))):
        assert compiled.execute(executable, stdin, seconds) == expected
    invalid = guest + b"(def unchecked () Int (missing 1))\n"
    assert compiled.execute(compiler, invalid, seconds) == (249, b"")
    print(f"Large Alpha tape used {len(program)} bytes and passed three guest outcomes and a late refusal.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--accelerated", action="store_true")
    compiled.add_invocation_budget(parser)
    parser.add_argument("--omega", action="store_true")
    parser.add_argument("--omega-values", action="store_true",
                        help="execute bootstrap parameter/local/call and refusal controls")
    parser.add_argument("--omega-control", action="store_true",
                        help="execute actual lexer predicates and typed runtime control checks")
    parser.add_argument("--omega-storage", action="store_true",
                        help="execute actual SourceUnit clear and record/reference controls")
    parser.add_argument("--omega-utf8", action="store_true",
                        help="execute the actual UTF-8 decoder and checked arithmetic controls")
    parser.add_argument("--large-alpha", action="store_true", help="compile and run a program beyond the plain tape limit")
    parser.add_argument("--compiler-only", action="store_true",
                        help="stop after compiler construction, rebuilds, and differential controls")
    args = parser.parse_args()
    if args.compiler_only and (args.omega or args.omega_values or args.omega_control or args.omega_storage or args.omega_utf8):
        parser.error("--compiler-only and Omega customers select incompatible scopes")
    mode = "accelerated" if args.accelerated else "reference"
    work = compiled.ROOT / "build" / ("source-gamma-" + mode)
    work.mkdir(parents=True, exist_ok=True)
    source = compiled.gamma_backend_source("alpha")
    (work / "compiler.gamma").write_bytes(source)
    compiled.reconstruct(work, seconds=args.seconds)
    request = compiled.gamma_request(source, source)
    (work / "construction.request").write_bytes(request)
    budget = "no wall limit" if args.seconds is None else f"{args.seconds}s wall limit"
    print(f"{mode} construction: source={len(source)} bytes sha256={digest(source)} ({budget})", flush=True)
    started = time.monotonic()
    if args.accelerated:
        import gamma
        sys.setrecursionlimit(4096)
        development = compiled.compile_program(work, source, "development", compiler=gamma.Compiler)
        status, tape = compiled.execute(development, source, args.seconds)
    else:
        status, tape = compiled.execute(work / "reference", request, args.seconds)
    assert status == 0, ("compiler construction refused", status, len(tape))
    (work / "compiler0.tape").write_bytes(tape)
    compiled.stamp(work / "compiler0.tape", work / "compiler0")
    print(f"{mode} compiler0: {len(tape)} bytes sha256={digest(tape)}; "
          f"{time.monotonic()-started:.3f}s", flush=True)
    for generation in (1, 2):
        status, rebuilt = compiled.execute(work / f"compiler{generation-1}", source, args.seconds)
        assert status == 0 and rebuilt == tape, ("compiler fixed point", generation, status)
        (work / f"compiler{generation}.tape").write_bytes(rebuilt)
        compiled.stamp(work / f"compiler{generation}.tape", work / f"compiler{generation}")
    print("compiler0 = compiler1 = compiler2: byte-exact", flush=True)

    class SourceCompiler:
        def __init__(self, subject):
            self.subject = subject

        def compile(self):
            status, result = compiled.execute(work / "compiler2", self.subject, args.seconds)
            if status:
                assert result == b"", "compiler refusal published partial bytes"
                assert status == 249, ("compiler physical/internal failure", status)
                raise compiled.Invalid("Gamma source compiler rejected malformed source")
            return result

    compiled.controls(work, work / "reference", compiler=SourceCompiler, seconds=args.seconds)
    compiled.call_depth_controls(work, SourceCompiler, args.seconds)
    if args.large_alpha:
        large_tape_control(work, work / "compiler2", args.seconds)
    if args.compiler_only:
        print(f"{mode} compiler construction and controls passed", flush=True)
        return
    compiled.customer(work, work / "reference", producer=SourceCompiler, seconds=args.seconds)
    compiled.epsilon_customer(work, producer=SourceCompiler, seconds=args.seconds)
    if args.omega:
        compiled.omega_customer(work, args.seconds)
    if args.omega_values:
        compiled.omega_values_customer(work, args.seconds)
    if args.omega_control:
        compiled.omega_control_customer(work, args.seconds)
    if args.omega_storage:
        compiled.omega_storage_customer(work, args.seconds)
    if args.omega_utf8:
        compiled.omega_utf8_customer(work, args.seconds)
    print(f"{mode} construction and requested customers passed", flush=True)


if __name__ == "__main__":
    main()
