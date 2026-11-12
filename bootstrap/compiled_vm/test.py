#!/usr/bin/env python3
"""Differential compiled-Gamma checks and the real Delta compiler customer.

Uses the existing Alpha stamping and source-closure owners. No host evaluator
stands in for Gamma or Delta execution. Each native process has a timeout.
"""
import argparse
import hashlib
import importlib.util
import os
from pathlib import Path
import struct
import subprocess
import sys
import time

import gamma
from gamma import Compiler, Invalid

ROOT = Path(__file__).resolve().parents[2]
sys.setrecursionlimit(4096)


def shell(script, *args):
    subprocess.run(["sh", "-eu", "-c", script, "compiled-vm", *map(str, args)],
                   cwd=ROOT, env={**os.environ, "OMEGA_REPO_ROOT": str(ROOT)}, check=True)


def stamp(tape, executable):
    shell('. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"; '
          '. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"; '
          'stamp_seed "$1" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$2"', tape, executable)


def execute(executable, payload=b"", timeout=60):
    result = subprocess.run([str(executable)], input=payload, capture_output=True,
                            timeout=timeout)
    if result.stderr:
        raise AssertionError(result.stderr)
    return result.returncode, result.stdout


def reference(evaluator, source, payload=b""):
    return execute(evaluator, struct.pack("<I", len(source)) + source + payload)


def compile_program(work, source, name="subject", compiler=Compiler):
    tape = work / (name + ".tape")
    tape.write_bytes(compiler(source).compile())
    executable = work / name
    stamp(tape, executable)
    return executable


def controls(work, evaluator, compiler=Compiler):
    cases = [
        ("constant", "(def main () Int 42)", b"", (0, b"*")),
        ("wrap", "(def main () Int (+ 18446744073709551615 43))", b"", (0, b"*")),
        ("signed", "(def main () Int (+ (/ -85 2) 84))", b"", (0, b"*")),
        ("modulo", "(def main () Int (+ (% -85 2) 43))", b"", (0, b"*")),
        ("character", "(def main () Int ' ')", b"", (0, b" ")),
        ("grouped", "(def main () Int (let ((first Int 40) (second Int (+ first 2))) second))", b"", (0, b"*")),
        ("binder-let", "(def main () Int (let ((let Int 42)) let))", b"", (0, b"*")),
        ("minimum-word", "(def main () Int (+ 41 (lt -9223372036854775808 0)))", b"", (0, b"*")),
        ("cr-comment", "(def main () Int (f)); comment\r(def f () Int 42)", b"", (0, b"*")),
        ("nested-pair", "(def main () Int (first (second (pair 0 (pair 42 9)))))", b"", (0, b"*")),
        ("order", "(def f ((a Int) (b Int)) Int (+ a b)) (def main () Int (f (write 20) (write 22)))", b"", (0, bytes([20,22,42]))),
        ("sealed", "(def main () Int (+ (input) (read 1)))", b"AB", (0, b"D")),
        ("recursive", "(def sum ((n Int)) Int (if (eq n 0) 0 (+ n (sum (- n 1))))) (def main () Int (sum 7))", b"", (0, bytes([28]))),
        ("tail-swap", "(def loop ((n Int) (a Int) (b Int)) Int (if (eq n 0) a (loop (- n 1) b a))) (def main () Int (loop 100001 41 42))", b"", (0, b"*")),
        ("tail-arity", "(def one ((n Int)) Int (if (eq n 0) 42 (two (- n 1) 9))) (def two ((n Int) (x Int)) Int (one n)) (def main () Int (one 10000))", b"", (0, b"*")),
        ("tail-nested", "(def f ((n Int)) Int (if (eq n 0) 40 (f (- n 1)))) (def main () Int (let x Int 2 (+ (f 100) x)))", b"", (0, b"*")),
        ("tail-pair", "(def f ((p Int) (n Int)) Int (if (eq n 0) p (g n p 7))) (def g ((n Int) (p Int) (x Int)) Int (let next Int (- n 1) (f p next))) (def main () Int (first (f (pair 42 43) 100)))", b"", (0, b"*")),
        ("bad-projection", "(def main () Int (first 42))", b"", (2, b"")),
        ("tagged-condition", "(def main () Int (if (pair 0 0) 42 41))", b"", (2, b"")),
        ("tagged-arithmetic", "(def main () Int (+ (pair 0 0) 42))", b"", (2, b"")),
        ("tagged-io", "(def main () Int (write (pair 0 0)))", b"", (2, b"")),
        ("pair-main", "(def main () Int (pair 0 1))", b"", (2, b"")),
        ("atomic", "(def main () Int (let x Int (write 42) (/ x 0)))", b"", (2, b"")),
        ("divide-overflow", "(def main () Int (/ -9223372036854775808 -1))", b"", (2, b"")),
        ("mod-overflow", "(def main () Int (% -9223372036854775808 -1))", b"", (2, b"")),
        ("read-negative", "(def main () Int (read -1))", b"A", (2, b"")),
        ("read-end", "(def main () Int (read 1))", b"A", (2, b"")),
        ("write-range", "(def main () Int (write 256))", b"", (2, b"")),
        ("app-publish", "(def $application () Int 1) (def main () Int (let x Int (write 42) (pair 7 1)))", b"", (7, b"*")),
        ("app-discard", "(def $application () Int 1) (def main () Int (let x Int (write 42) (pair 7 0)))", b"", (7, b"")),
        ("app-trap", "(def $application () Int 1) (def main () Int (let x Int (write 42) (pair 0 0)))", b"", (249, b"")),
        ("app-empty-error", "(def $application () Int 1) (def main () Int (pair 7 1))", b"", (249, b"")),
    ]
    for label, text, payload, expected in cases:
        source = text.encode("ascii")
        actual = execute(compile_program(work, source, compiler=compiler), payload)
        original = reference(evaluator, source, payload)
        assert actual == original == expected, (label, actual, original, expected)
    invalid = [
        b"(def main () Int 42", b"(def main () Int 42)\x00",
        b"(def main () Int 42) (def main () Int 43)",
        b"(def main ((x Int)) Int x)", b"(def main () Bytes 42)",
        b"(def main () Int (if 1 42 unknown))",
        b"(def main () Int 42) (def unused () Int (missing 1))",
        b"(def main () Int (let x Int x x))",
        b"(def main () Int (let ((x Int 1) (x Int 2)) x))",
        b"(def f ((x Int) (x Int)) Int x) (def main () Int 42)",
        b"(def f ((x Int)) Int x) (def main () Int (f))",
        b"(def main () Int (+ 1))",
        b"(def main () Int 42);comment\r(def unused () Wrong 0)",
        b"(def main () Int ''')", b"(def main () Int '\\')",
    ]
    for source in invalid:
        try:
            compiler(source).compile()
        except Invalid:
            pass
        else:
            raise AssertionError(("invalid source compiled", source))
        assert reference(evaluator, source) == (1, b""), source
    if compiler is not Compiler:
        print(f"controls: {len(cases)} exact native differential observations; {len(invalid)} rejected sources", flush=True)
        return
    (work / "invalid.gamma").write_bytes(invalid[0])
    (work / "existing.tape").write_bytes(b"previous artifact")
    refused = subprocess.run([sys.executable, str(ROOT / "bootstrap/compiled_vm/gamma.py"),
                              str(work / "invalid.gamma"), str(work / "existing.tape")],
                             capture_output=True, timeout=60)
    assert refused.returncode == 1 and not refused.stdout
    assert (work / "existing.tape").read_bytes() == b"previous artifact"
    print(f"controls: {len(cases)} exact native differential observations; {len(invalid)} rejected sources", flush=True)

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
            actual = execute(compile_program(work, source, compiler=compiler), payload)
        finally:
            setattr(gamma, field, previous)
        assert actual == expected, (field, actual, expected)
    print(f"controls: {len(bounds)} compiled storage-boundary observations; refused CLI preserves prior artifact", flush=True)


def pack(manifest, output, prefix=None):
    command = [sys.executable, str(ROOT / "tools/bootstrap/source_closure.py"),
               str(ROOT / manifest), str(output)]
    if prefix:
        command.extend(["--prefix", str(ROOT / prefix)])
    subprocess.run(command, check=True)
    return output.read_bytes()


def customer(work, evaluator, producer=Compiler):
    source = pack("bootstrap/3_delta/implementation/implementation.gamma.sources",
                  work / "delta.gamma", "bootstrap/3_delta/delta_compiler.gamma")
    support = pack("bootstrap/3_delta/support/support.gamma.sources", work / "support.bin")
    started = time.monotonic()
    compiler = compile_program(work, source, "delta", compiler=producer)
    print(f"Delta customer: {len(source)} source bytes -> {(work / 'delta.tape').stat().st_size} tape bytes; compilation {time.monotonic()-started:.3f}s", flush=True)
    delta = b"(def main ((source Bytes)) Bytes (bytes_concat source source))\n"
    request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(delta)) + delta + support
    (work / "request.bin").write_bytes(request)
    started = time.monotonic()
    actual = execute(compiler, request)
    elapsed = time.monotonic() - started
    original = reference(evaluator, source, request)
    assert actual == original and actual[0] == 0, (actual[0], original[0], len(actual[1]), len(original[1]))
    receipt = actual[1]
    (work / "receipt.gamma").write_bytes(receipt)
    result = compile_program(work, receipt, "receipt", compiler=producer)
    for payload in (b"", b"ABC", bytes(range(256))):
        assert execute(result, payload) == reference(evaluator, receipt, payload) == (0, payload * 2)
    for request in (b"", b"DCREQ\x01\x00\x00" + struct.pack("<II", 0, 0)):
        assert execute(compiler, request) == reference(evaluator, source, request)
    print(f"Delta customer: exact receipt {len(receipt)} bytes sha256={hashlib.sha256(receipt).hexdigest()}; compiled execution {elapsed:.3f}s (functional run, not a comparative benchmark)", flush=True)


def epsilon_customer(work, producer=Compiler):
    closure = pack("bootstrap/4_epsilon/epsilon_compiler.delta.sources", work / "epsilon.delta")
    subject = closure + (ROOT / "tests/epsilon/evaluator-entry/evaluator_entry.delta").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + (work / "support.bin").read_bytes())
    started = time.monotonic()
    status, receipt = execute(work / "delta", request)
    assert status == 0, (status, receipt[:40])
    elapsed = time.monotonic() - started
    (work / "epsilon.gamma").write_bytes(receipt)
    shell('. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"; '
          '. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"; '
          'require_epsilon_evaluator_entry_receipt_identity "$1"', work / "epsilon.gamma")
    executable = compile_program(work, receipt, "epsilon", compiler=producer)
    console = b"""boundary trait Console {
  machine exit_process(return_code: i32) -> never;
  machine write_byte(value: i32);
  machine read_byte() -> i32;
  machine write_line(text: &[u8]);
}
data Main { console: Console; }
machine Main::main(&mut self) {
"""
    cases = [
        (b"", b"", b"\x02\x08\x00\x00\x00\x00"),
        (console + b"self.console.write_byte(65); self.console.exit_process(42); }", b"", b"\x00" + struct.pack("<i", 42) + b"A"),
        (console + b"let x: i32 = self.console.read_byte(); self.console.write_byte(x); self.console.exit_process(x); }", b"Z", b"\x00" + struct.pack("<i", 90) + b"Z"),
        (console + b"self.console.write_byte(66); let x: i32 = 1 / 0; }", b"", b"\x01\x02B"),
    ]
    for source, stdin, expected in cases:
        request = epsilon_request(closure, source, stdin)
        assert execute(executable, request) == (0, expected)
    short = execute(executable, b"")
    assert short == (0, struct.pack("<8sBBHIQQQ", b"\xffEEOUT\x01\x00", 1, 1, 0, 1, 0, 0, 0))
    print(f"Epsilon customer: reconstructed bound {len(receipt)}-byte receipt in {elapsed:.3f}s; {(work / 'epsilon.tape').stat().st_size} Alpha bytes; exit, stdin, trap, reject and malformed-envelope observations passed", flush=True)


def epsilon_request(closure, source, stdin):
    return (b"EEREQ\x01\x00\x00" + struct.pack("<III", 1, len(source), len(stdin))
            + hashlib.sha256(closure).digest() + source + stdin)


def omega_customer(work, timeout, controls=False):
    closure = pack("bootstrap/5_omega/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/omega-executable"
    source = closure + (gate / ("controls_b.epsilon" if controls else "main_ocreq.epsilon")).read_bytes()
    # Reuse the existing byte-only request framer; it does not evaluate source.
    spec = importlib.util.spec_from_file_location("omega_fixture", gate / "gate.py")
    fixture = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fixture)
    stdin = b"" if controls else fixture.ocreq_request((gate / "program.omg").read_bytes(), b"program.omg")
    request = epsilon_request((work / "epsilon.delta").read_bytes(), source, stdin)
    label = "omega-controls" if controls else "omega"
    (work / (label + "-request.bin")).write_bytes(request)
    print(f"{label} customer: {len(source)} Epsilon source bytes; execution allowance {timeout}s", flush=True)
    started = time.monotonic()
    status, observation = execute(work / "epsilon", request, timeout)
    assert status == 0 and observation[:5] == bytes(5), (status, observation[:80])
    (work / (label + "-observation.bin")).write_bytes(observation)
    if controls:
        expected_tape = bytes.fromhex("13 0b00000000000000 00 00 01 00 2a00000000000000 14")
        assert observation == bytes(5) + b"C" + expected_tape
        tape = observation[6:]
    else:
        tape = observation[5:]
    (work / (label + ".tape")).write_bytes(tape)
    stamp(work / (label + ".tape"), work / (label + "-program"))
    expected = int((gate / "expected.txt").read_text())
    assert execute(work / (label + "-program")) == (expected, b"")
    print(f"{label} customer: {len(tape)} Alpha bytes -> exit {expected}; elapsed {time.monotonic()-started:.3f}s", flush=True)


def omega_values_customer(work, timeout):
    """Execute the source-owned parameter/local/call checks and their tapes."""
    closure = pack("bootstrap/5_omega/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    entry = (ROOT / "tests/bootstrap/omega-executable/controls_values.epsilon").read_bytes()
    source = closure + entry
    request = epsilon_request((work / "epsilon.delta").read_bytes(), source, b"")
    (work / "values-source.epsilon").write_bytes(source)
    (work / "values-request.bin").write_bytes(request)
    print(f"Omega values customer: {len(source)} source bytes; allowance {timeout}s", flush=True)
    started = time.monotonic()
    status, observation = execute(work / "epsilon", request, timeout)
    (work / "values-observation.bin").write_bytes(observation)
    assert status == 0 and observation[:5] == bytes(5), (status, observation[:80])
    offset = 5
    for index, expected in enumerate((42, 42, 53, 44, 52, 42, 56, 37, 0, 2, 47, 43)):
        assert offset + 5 <= len(observation), ("truncated artifact frame", index)
        code = observation[offset]
        length = struct.unpack_from("<I", observation, offset + 1)[0]
        offset += 5
        assert code == expected and 0 < length <= 16777212
        assert offset + length <= len(observation), ("truncated artifact", index)
        tape = work / f"values-{index}.tape"
        executable = work / f"values-{index}"
        tape.write_bytes(observation[offset:offset + length])
        offset += length
        stamp(tape, executable)
        assert execute(executable) == (expected, b""), index
    assert offset == len(observation), "unexpected publication after the final artifact"
    print(f"Omega values customer: twelve emitted executables and 33 source-owned refusal/reset checks passed; {time.monotonic()-started:.3f}s", flush=True)


def omega_control_customer(work, timeout):
    """Compile the repository lexer predicates and execute scalar controls."""
    closure = pack("bootstrap/5_omega/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/omega-executable"
    lexer = (ROOT / "source/psi/lex/lexer.omg").read_bytes()
    declarations = []
    for name in (b"is_identifier_start", b"is_identifier_continue"):
        start = lexer.index(b"machine " + name + b"(")
        end = lexer.index(b"\nmachine ", start + 1)
        declarations.append(lexer[start:end])
    program = b"\n".join(declarations) + b"\n" + (gate / "lexer_controls.omg").read_bytes()
    source = closure + (gate / "controls_control.epsilon").read_bytes()
    request = epsilon_request((work / "epsilon.delta").read_bytes(), source, program)
    (work / "control-source.epsilon").write_bytes(source)
    (work / "control-program.omg").write_bytes(program)
    (work / "control-request.bin").write_bytes(request)
    print(f"Omega runtime control: {len(source)} Epsilon bytes; {len(program)} Omega bytes; allowance {timeout}s", flush=True)
    started = time.monotonic()
    status, observation = execute(work / "epsilon", request, timeout)
    (work / "control-observation.bin").write_bytes(observation)
    assert status == 0 and observation[:5] == bytes(5), (status, observation[:80])
    offset = 5
    for index, expected in enumerate((42, 42, 43, 44, 45, 46, 47)):
        assert offset + 5 <= len(observation), ("truncated artifact frame", index)
        code = observation[offset]
        length = struct.unpack_from("<I", observation, offset + 1)[0]
        offset += 5
        assert code == expected and 0 < length <= 16777212
        assert offset + length <= len(observation), ("truncated artifact", index)
        tape = work / f"control-{index}.tape"
        executable = work / f"control-{index}"
        tape.write_bytes(observation[offset:offset + length])
        offset += length
        stamp(tape, executable)
        assert execute(executable) == (expected, b""), index
    assert offset == len(observation), "unexpected publication after the final artifact"
    print(f"Omega runtime control: seven emitted executables, actual lexer boundaries, parallel transfers, short circuit, explicit domain casts and 26 refusals passed; {time.monotonic()-started:.3f}s", flush=True)


def omega_storage_customer(work, timeout):
    """Compile exact SourceUnit declarations and execute generic storage checks."""
    closure = pack("bootstrap/5_omega/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/omega-executable"
    unit = (ROOT / "source/psi/source/source.omg").read_bytes()
    declarations = []
    for marker in (b"public data SourceId ", b"public data SourceUnit\n", b"machine source_unit_clear("):
        start = unit.index(marker)
        end = unit.index(b"\n}", start) + 2
        declarations.append(unit[start:end])
    program = b"\n\n".join(declarations) + b"\n" + (gate / "storage_controls.omg").read_bytes()
    source = closure + (gate / "controls_storage.epsilon").read_bytes()
    request = epsilon_request((work / "epsilon.delta").read_bytes(), source, program)
    (work / "storage-source.epsilon").write_bytes(source)
    (work / "storage-program.omg").write_bytes(program)
    (work / "storage-request.bin").write_bytes(request)
    print(f"Omega storage: {len(source)} Epsilon bytes; {len(program)} Omega bytes; allowance {timeout}s", flush=True)
    started = time.monotonic()
    status, observation = execute(work / "epsilon", request, timeout)
    (work / "storage-observation.bin").write_bytes(observation)
    assert status == 0 and observation[:5] == bytes(5), (status, observation[:80])
    offset = 5
    for index, expected in enumerate((42, 43, 44, 45, 46, 0, 2, 48, 44, 59)):
        assert offset + 5 <= len(observation), ("truncated artifact frame", index)
        code = observation[offset]
        length = struct.unpack_from("<I", observation, offset + 1)[0]
        offset += 5
        assert code == expected and 0 < length <= 16777212
        assert offset + length <= len(observation), ("truncated artifact", index)
        tape = work / f"storage-{index}.tape"
        executable = work / f"storage-{index}"
        tape.write_bytes(observation[offset:offset + length])
        offset += length
        stamp(tape, executable)
        assert execute(executable) == (expected, b""), index
    assert offset == len(observation), "unexpected publication after the final artifact"
    print(f"Omega storage: ten executables, actual SourceUnit clear with byte canary and 27 refusal/reset checks passed; {time.monotonic()-started:.3f}s", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--customer-only", action="store_true")
    parser.add_argument("--controls-only", action="store_true")
    parser.add_argument("--epsilon", action="store_true", help="construct and execute the complete Epsilon evaluator")
    parser.add_argument("--omega", action="store_true", help="also run the complete bootstrap compiler's existing executable fixture")
    parser.add_argument("--omega-controls", action="store_true", help="run its existing changed-source, rejection, and reset controls")
    parser.add_argument("--omega-values", action="store_true", help="run the parameter, local, nested-call and refusal customer")
    parser.add_argument("--omega-control", action="store_true", help="run actual lexer predicates and typed runtime control checks")
    parser.add_argument("--omega-storage", action="store_true", help="run actual SourceUnit clear and record/reference controls")
    parser.add_argument("--omega-timeout", type=int,
                        help="Omega observation seconds (default: 7200 for runtime/storage controls, otherwise 1800)")
    args = parser.parse_args()
    if args.omega_timeout is None:
        args.omega_timeout = 7200 if args.omega_control or args.omega_storage else 1800
    if args.omega_timeout <= 0:
        parser.error("--omega-timeout must be positive")
    work = ROOT / "build/compiled-vm"
    work.mkdir(parents=True, exist_ok=True)
    evaluator = work / "reference"
    shell('. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"; '
          '. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"; '
          'materialize_gamma_evaluator "$1"', evaluator)
    if not args.customer_only:
        controls(work, evaluator)
    if not args.controls_only:
        customer(work, evaluator)
        if args.epsilon or args.omega or args.omega_controls or args.omega_values or args.omega_control or args.omega_storage:
            epsilon_customer(work)
        if args.omega:
            omega_customer(work, args.omega_timeout)
        if args.omega_controls:
            omega_customer(work, args.omega_timeout, controls=True)
        if args.omega_values:
            omega_values_customer(work, args.omega_timeout)
        if args.omega_control:
            omega_control_customer(work, args.omega_timeout)
        if args.omega_storage:
            omega_storage_customer(work, args.omega_timeout)


if __name__ == "__main__":
    main()
