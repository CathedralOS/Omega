#!/usr/bin/env python3
"""Differential compiled-Gamma checks and the real Delta compiler customer.

Uses the existing Alpha stamping and source-closure owners. No host evaluator
stands in for Gamma or Delta execution. Each native process has a timeout.
"""
import argparse
import subprocess
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tests"))
import compiled

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import gamma

sys.setrecursionlimit(4096)


def host_compiler_controls(work):
    (work / "invalid.gamma").write_bytes(compiled.INVALID_GAMMA[0])
    (work / "existing.tape").write_bytes(b"previous artifact")
    refused = subprocess.run([sys.executable, str(compiled.ROOT / "bootstrap/compiled_vm/gamma.py"),
                              str(work / "invalid.gamma"), str(work / "existing.tape")],
                             capture_output=True, timeout=60)
    assert refused.returncode == 1 and not refused.stdout
    assert (work / "existing.tape").read_bytes() == b"previous artifact"

    # Exercise generated physical guards with small, disjoint test regions.
    # These are compiled-profile observations, not reference capacity claims.
    bounds = [("INPUT_END", gamma.INPUT + 2, b"(def main () Int (input))", b"AB", (0, b"\x02")),
              ("INPUT_END", gamma.INPUT + 2, b"(def main () Int 0)", b"ABC", (3, b"")),
              ("INPUT_END", gamma.INPUT + 2, b"(def $application () Int 1) (def main () Int (pair 0 1))", b"ABC", (253, b"")),
              ("OUTPUT_END", gamma.OUTPUT + 2, b"(def main () Int (let x Int (write 65) 66))", b"", (0, b"AB")),
              ("OUTPUT_END", gamma.OUTPUT + 2, b"(def main () Int (let ((x Int (write 65)) (y Int (write 66))) 67))", b"", (3, b"")),
              ("HEAP_END", gamma.HEAP + 32, b"(def main () Int (first (pair 42 43)))", b"", (0, b"*")),
              ("HEAP_END", gamma.HEAP + 32, b"(def main () Int (first (pair (pair 42 43) 44)))", b"", (3, b"")),
              ("STACK_END", gamma.STACK + 32, b"(def main () Int 42)", b"", (0, b"*")),
              ("STACK_END", gamma.STACK + 32, b"(def main () Int (+ 20 22))", b"", (3, b""))]
    for field, limit, source, payload, expected in bounds:
        previous = getattr(gamma, field)
        try:
            setattr(gamma, field, limit)
            actual = compiled.execute(compiled.compile_program(work, source, compiler=gamma.Compiler), payload)
        finally:
            setattr(gamma, field, previous)
        assert actual == expected, (field, actual, expected)
    print(f"controls: {len(bounds)} compiled storage-boundary observations; refused CLI preserves prior artifact", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--customer-only", action="store_true")
    parser.add_argument("--controls-only", action="store_true")
    parser.add_argument("--epsilon", action="store_true", help="construct and execute the complete Epsilon evaluator")
    parser.add_argument("--omega", action="store_true", help="also run the complete bootstrap compiler's existing executable fixture")
    parser.add_argument("--omega-controls", action="store_true", help="run its existing changed-source, rejection, and reset controls")
    parser.add_argument("--omega-values", action="store_true", help="run the parameter, local, nested-call and refusal customer")
    parser.add_argument("--omega-control", action="store_true", help="run actual lexer predicates and typed runtime control checks")
    parser.add_argument("--omega-storage", action="store_true", help="run actual SourceUnit append/read and checked storage controls")
    parser.add_argument("--omega-utf8", action="store_true", help="run the real UTF-8 decoder and checked arithmetic boundaries")
    parser.add_argument("--omega-timeout", type=int,
                        help="Omega observation seconds (default: 7200 for runtime/storage controls, otherwise 1800)")
    args = parser.parse_args()
    if args.omega_timeout is None:
        args.omega_timeout = 7200 if args.omega_control or args.omega_storage or args.omega_utf8 else 1800
    if args.omega_timeout <= 0:
        parser.error("--omega-timeout must be positive")
    work = compiled.ROOT / "build/compiled-vm"
    work.mkdir(parents=True, exist_ok=True)
    evaluator = work / "reference"
    compiled.shell('. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"; '
          '. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"; '
          'materialize_gamma_evaluator "$1"', evaluator)
    if not args.customer_only:
        compiled.controls(work, evaluator, gamma.Compiler, invalid=gamma.Invalid)
        host_compiler_controls(work)
    if not args.controls_only:
        compiled.customer(work, evaluator, gamma.Compiler)
        if args.epsilon or args.omega or args.omega_controls or args.omega_values or args.omega_control or args.omega_storage or args.omega_utf8:
            compiled.epsilon_customer(work, gamma.Compiler)
        if args.omega:
            compiled.omega_customer(work, args.omega_timeout)
        if args.omega_controls:
            compiled.omega_customer(work, args.omega_timeout, controls=True)
        if args.omega_values:
            compiled.omega_values_customer(work, args.omega_timeout)
        if args.omega_control:
            compiled.omega_control_customer(work, args.omega_timeout)
        if args.omega_storage:
            compiled.omega_storage_customer(work, args.omega_timeout)
        if args.omega_utf8:
            compiled.omega_utf8_customer(work, args.omega_timeout)


if __name__ == "__main__":
    main()
