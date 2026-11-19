#!/usr/bin/env python3
"""Construct the Gamma compiler through Beta/Gamma, rebuild, and run customers.

Python frames bytes and compares observations. Only --accelerated invokes the
host Gamma compiler as a semantic producer; its results are labeled separately.
"""

import argparse
import hashlib
from pathlib import Path
import struct
import time

import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
import compiled


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--accelerated", action="store_true")
    parser.add_argument("--seconds", type=int,
                        help="observation seconds (default: 7200 with runtime/storage controls, otherwise 3600)")
    parser.add_argument("--omega", action="store_true")
    parser.add_argument("--omega-values", action="store_true",
                        help="execute bootstrap parameter/local/call and refusal controls")
    parser.add_argument("--omega-control", action="store_true",
                        help="execute actual lexer predicates and typed runtime control checks")
    parser.add_argument("--omega-storage", action="store_true",
                        help="execute actual SourceUnit clear and record/reference controls")
    parser.add_argument("--omega-utf8", action="store_true",
                        help="execute the actual UTF-8 decoder and checked arithmetic controls")
    parser.add_argument("--compiler-only", action="store_true",
                        help="stop after compiler construction, rebuilds, and differential controls")
    args = parser.parse_args()
    if args.seconds is None:
        args.seconds = 7200 if args.omega_control or args.omega_storage or args.omega_utf8 else 3600
    if args.seconds <= 0:
        parser.error("--seconds must be positive")
    if args.compiler_only and (args.omega or args.omega_values or args.omega_control or args.omega_storage or args.omega_utf8):
        parser.error("--compiler-only and Omega customers select incompatible scopes")
    mode = "accelerated" if args.accelerated else "reference"
    work = compiled.ROOT / "build" / ("source-gamma-" + mode)
    work.mkdir(parents=True, exist_ok=True)
    source = b"\n".join((compiled.ROOT / "bootstrap/compiled_vm" / name).read_bytes()
                         for name in ("compiler.gamma", "frontend.gamma", "alpha_backend.gamma"))
    (work / "compiler.gamma").write_bytes(source)
    compiled.reconstruct(work)
    request = struct.pack("<I", len(source)) + source + source
    (work / "construction.request").write_bytes(request)
    print(f"{mode} construction: source={len(source)} bytes sha256={digest(source)}; "
          f"allowance={args.seconds}s", flush=True)
    started = time.monotonic()
    if args.accelerated:
        sys.path.insert(0, str(compiled.ROOT / "bootstrap/compiled_vm"))
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

    compiled.controls(work, work / "reference", compiler=SourceCompiler)
    if args.compiler_only:
        print(f"{mode} compiler construction and controls passed", flush=True)
        return
    compiled.customer(work, work / "reference", producer=SourceCompiler)
    compiled.epsilon_customer(work, producer=SourceCompiler)
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
