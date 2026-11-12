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

import test


def digest(data):
    return hashlib.sha256(data).hexdigest()


def reconstruct(work):
    test.shell('''
      . "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
      . "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
      . "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
      materialize_beta_compiler "$1/beta-seed"
      "$1/beta-seed" < "$OMEGA_PATH_BETA_COMPILER_SOURCE" > "$1/beta.tape"
      cmp "$1/beta.tape" "$OMEGA_PATH_BETA_COMPILER_TAPE"
      stamp_seed "$1/beta.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$1/beta"
      require_gamma_evaluator_identity
      "$1/beta" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$1/gamma.tape"
      cmp "$1/gamma.tape" "$OMEGA_PATH_GAMMA_EVALUATOR_TAPE"
      stamp_seed "$1/gamma.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$1/reference"
    ''', work)
    print("Beta self-reconstruction and Gamma source construction: byte-exact", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--accelerated", action="store_true")
    parser.add_argument("--seconds", type=int, default=3600)
    parser.add_argument("--omega", action="store_true")
    parser.add_argument("--omega-values", action="store_true",
                        help="execute bootstrap parameter/local/call and refusal controls")
    parser.add_argument("--compiler-only", action="store_true",
                        help="stop after compiler construction, rebuilds, and differential controls")
    args = parser.parse_args()
    if args.seconds <= 0:
        parser.error("--seconds must be positive")
    if args.compiler_only and (args.omega or args.omega_values):
        parser.error("--compiler-only and Omega customers select incompatible scopes")
    mode = "accelerated" if args.accelerated else "reference"
    work = test.ROOT / "build" / ("source-gamma-" + mode)
    work.mkdir(parents=True, exist_ok=True)
    source = b"\n".join((test.ROOT / "bootstrap/compiled_vm" / name).read_bytes()
                         for name in ("compiler.gamma", "frontend.gamma"))
    (work / "compiler.gamma").write_bytes(source)
    reconstruct(work)
    request = struct.pack("<I", len(source)) + source + source
    (work / "construction.request").write_bytes(request)
    print(f"{mode} construction: source={len(source)} bytes sha256={digest(source)}; "
          f"allowance={args.seconds}s", flush=True)
    started = time.monotonic()
    if args.accelerated:
        development = test.compile_program(work, source, "development")
        status, tape = test.execute(development, source, args.seconds)
    else:
        status, tape = test.execute(work / "reference", request, args.seconds)
    assert status == 0, ("compiler construction refused", status, len(tape))
    (work / "compiler0.tape").write_bytes(tape)
    test.stamp(work / "compiler0.tape", work / "compiler0")
    print(f"{mode} compiler0: {len(tape)} bytes sha256={digest(tape)}; "
          f"{time.monotonic()-started:.3f}s", flush=True)
    for generation in (1, 2):
        status, rebuilt = test.execute(work / f"compiler{generation-1}", source, args.seconds)
        assert status == 0 and rebuilt == tape, ("compiler fixed point", generation, status)
        (work / f"compiler{generation}.tape").write_bytes(rebuilt)
        test.stamp(work / f"compiler{generation}.tape", work / f"compiler{generation}")
    print("compiler0 = compiler1 = compiler2: byte-exact", flush=True)

    class SourceCompiler:
        def __init__(self, subject):
            self.subject = subject

        def compile(self):
            status, result = test.execute(work / "compiler2", self.subject, args.seconds)
            if status:
                assert result == b"", "compiler refusal published partial bytes"
                assert status == 249, ("compiler physical/internal failure", status)
                raise test.Invalid("Gamma source compiler rejected malformed source")
            return result

    test.controls(work, work / "reference", compiler=SourceCompiler)
    if args.compiler_only:
        print(f"{mode} compiler construction and controls passed", flush=True)
        return
    test.customer(work, work / "reference", producer=SourceCompiler)
    test.epsilon_customer(work, producer=SourceCompiler)
    if args.omega:
        test.omega_customer(work, args.seconds)
    if args.omega_values:
        test.omega_values_customer(work, args.seconds)
    print(f"{mode} construction and requested customers passed", flush=True)


if __name__ == "__main__":
    main()
