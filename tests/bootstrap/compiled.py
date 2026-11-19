"""Shared construction and comparison helpers for the compiled bootstrap routes."""
import hashlib
import os
from pathlib import Path
import struct
import subprocess
import time

from conformance.vectors import GAMMA_CASES, INVALID_GAMMA, epsilon_observations, epsilon_request, ocreq_request

ROOT = Path(__file__).resolve().parents[2]


class Invalid(ValueError):
    """A source-constructed compiler rejected the supplied program."""


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


def compile_program(work, source, name="subject", *, compiler, realize=stamp):
    tape = work / (name + (".tape" if realize is stamp else ".elf"))
    tape.write_bytes(compiler(source).compile())
    executable = work / name
    realize(tape, executable)
    return executable


def controls(work, evaluator, compiler, realize=stamp, invalid=Invalid):
    for label, text, payload, expected in GAMMA_CASES:
        source = text.encode("ascii")
        actual = execute(compile_program(work, source, compiler=compiler, realize=realize), payload)
        original = reference(evaluator, source, payload)
        assert actual == original == expected, (label, actual, original, expected)
    for source in INVALID_GAMMA:
        try:
            compiler(source).compile()
        except invalid:
            pass
        else:
            raise AssertionError(("invalid source compiled", source))
        assert reference(evaluator, source) == (1, b""), source
    print(f"controls: {len(GAMMA_CASES)} exact differential observations; {len(INVALID_GAMMA)} rejected sources", flush=True)


def pack(manifest, output, prefix=None):
    binary = os.environ.get("OMEGA_XTASK_BIN") or str(ROOT / "target" / "debug" / "xtask")
    if not os.access(binary, os.X_OK):
        raise SystemExit(f"bootstrap artifact: xtask binary is not executable at {binary}")
    command = [binary, "source-closure", str(ROOT / manifest), str(output)]
    if prefix:
        command.extend(["--prefix", str(ROOT / prefix)])
    subprocess.run(command, check=True)
    return output.read_bytes()


def customer(work, evaluator, producer, realize=stamp):
    source = pack("bootstrap/interpreted/3_delta/implementation/implementation.gamma.sources",
                  work / "delta.gamma", "bootstrap/interpreted/3_delta/delta_compiler.gamma")
    support = pack("bootstrap/interpreted/3_delta/support/support.gamma.sources", work / "support.bin")
    started = time.monotonic()
    compiler = compile_program(work, source, "delta", compiler=producer, realize=realize)
    artifact = work / ("delta.tape" if realize is stamp else "delta.elf")
    print(f"Delta customer: {len(source)} source bytes -> {artifact.stat().st_size} artifact bytes; compilation {time.monotonic()-started:.3f}s", flush=True)
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
    result = compile_program(work, receipt, "receipt", compiler=producer, realize=realize)
    for payload in (b"", b"ABC", bytes(range(256))):
        assert execute(result, payload) == reference(evaluator, receipt, payload) == (0, payload * 2)
    for request in (b"", b"DCREQ\x01\x00\x00" + struct.pack("<II", 0, 0)):
        assert execute(compiler, request) == reference(evaluator, source, request)
    print(f"Delta customer: exact receipt {len(receipt)} bytes sha256={hashlib.sha256(receipt).hexdigest()}; compiled execution {elapsed:.3f}s (functional run, not a comparative benchmark)", flush=True)


def epsilon_customer(work, producer, realize=stamp):
    closure = pack("bootstrap/interpreted/4_epsilon/epsilon_compiler.delta.sources", work / "epsilon.delta")
    subject = closure + (ROOT / "tests/bootstrap/conformance/epsilon/evaluator_entry.delta").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + (work / "support.bin").read_bytes())
    (work / "epsilon-request.bin").write_bytes(request)
    started = time.monotonic()
    status, receipt = execute(work / "delta", request)
    assert status == 0, (status, receipt[:40])
    elapsed = time.monotonic() - started
    (work / "epsilon.gamma").write_bytes(receipt)
    shell('. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"; '
          '. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"; '
          'require_epsilon_evaluator_entry_receipt_identity "$1"', work / "epsilon.gamma")
    executable = compile_program(work, receipt, "epsilon", compiler=producer, realize=realize)
    for label, request, expected in epsilon_observations(closure):
        actual = execute(executable, request)
        assert actual == expected, (label, actual, expected)
    artifact = work / ("epsilon.tape" if realize is stamp else "epsilon.elf")
    print(f"Epsilon customer: reconstructed bound {len(receipt)}-byte receipt in {elapsed:.3f}s; {artifact.stat().st_size} artifact bytes; exit, stdin, trap, reject and malformed-envelope observations passed", flush=True)


def omega_customer(work, timeout, controls=False):
    closure = pack("bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/conformance/omega"
    source = closure + (gate / ("controls_b.epsilon" if controls else "main_ocreq.epsilon")).read_bytes()
    stdin = b"" if controls else ocreq_request((gate / "program.omg").read_bytes(), b"program.omg")
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
    closure = pack("bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    entry = (ROOT / "tests/bootstrap/conformance/omega/controls_values.epsilon").read_bytes()
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
    closure = pack("bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/conformance/omega"
    lexer = (ROOT / "source/subsystems/compiler/source_to_psi/pipeline/source_files_to_tokens/lexer.omg").read_bytes()
    declarations = []
    for name in (b"is_identifier_start", b"is_identifier_continue", b"lexer_digit_in_base"):
        start = lexer.index(b"machine " + name + b"(")
        end = lexer.index(b"\nmachine ", start + 1)
        declarations.append(lexer[start:end])
    tokens = (ROOT / "source/subsystems/compiler/source_to_psi/representations/tokens/tokens.omg").read_bytes()
    start = tokens.index(b"public data NumericBase ")
    end = tokens.index(b"\n}", start) + 2
    declarations.insert(0, tokens[start:end])
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
    for index, expected in enumerate((42, 42, 43, 44, 45, 46, 47, 42, 43, 43, 43, 42, 43, 42, 43, 42, 42, 42, 42)):
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
    print(f"Omega runtime control: 19 emitted executables, actual lexer byte/base matrix, nominal sums, source-set lookup/reset and 37 refusals passed; {time.monotonic()-started:.3f}s", flush=True)


def omega_storage_customer(work, timeout):
    """Compile exact SourceUnit declarations and execute generic storage checks."""
    closure = pack("bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/conformance/omega"
    unit = (ROOT / "source/subsystems/compiler/source_to_psi/representations/source_files/source.omg").read_bytes()
    declarations = []
    for marker in (b"public data SourceId ", b"public data SourceUnit\n", b"machine source_unit_clear(",
                   b"machine source_unit_append(", b"machine source_unit_byte_or_nul(",
                   b"public data ModulePath\n", b"public machine module_path_append("):
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
    for index, expected in enumerate((42, 43, 44, 45, 46, 0, 2, 48, 44, 0, 51, 53, 59)):
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
    print(f"Omega storage: thirteen executables, actual SourceUnit append/read and ModulePath boundaries, 36 refusal/reset checks passed; {time.monotonic()-started:.3f}s", flush=True)


def omega_utf8_customer(work, timeout, batch=None):
    """Compile the real UTF-8 decoder and execute arithmetic/trap boundaries."""
    closure = pack("bootstrap/interpreted/5_omega/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/conformance/omega"
    unit = (ROOT / "source/subsystems/compiler/source_to_psi/representations/source_files/source.omg").read_bytes()
    lexer = (ROOT / "source/subsystems/compiler/source_to_psi/pipeline/source_files_to_tokens/lexer.omg").read_bytes()
    declarations = []
    for source, markers in ((unit, (b"public data SourceId ", b"public data SourceUnit\n",
                                    b"machine source_unit_append(", b"machine source_unit_byte_or_nul(")),
                            (lexer, (b"data DecodedScalar ", b"machine decode_scalar("))):
        for marker in markers:
            start = source.index(marker)
            end = source.index(b"\n}", start) + 2
            declarations.append(source[start:end])
    program = b"\n\n".join(declarations) + b"\n" + (gate / "utf8_controls.omg").read_bytes()
    source = closure + (gate / "controls_arithmetic.epsilon").read_bytes()
    (work / "utf8-source.epsilon").write_bytes(source)
    (work / "utf8-program.omg").write_bytes(program)
    # Fresh evaluator processes bound cumulative immutable allocation without
    # changing compiler inputs or the lifetime of any refusal/reset sequence.
    # Each refusal batch runs positives before and after the refusals on the same compiler.
    expected_batches = (
        (42,),
        (42, -4, -4, -4, 42, 42, 42),
        (42, -4, -4),
        (42, -4, 59),
        (42, 43, 44, 59),
        (42, 42, 43, 44, 43, 45, 59),
        (42, 42, 42, 42),
        (42, -4, 43, 43, 44),
        (42, 44),
        (42, 42, 43, 44, 45),
        (42, -4, 42, 42),
    )
    batches = range(len(expected_batches)) if batch is None else (batch,)
    for selected in batches:
        assert 0 <= selected < len(expected_batches), selected
        payload = bytes([selected]) + (program if selected == 0 else b"")
        request = epsilon_request((work / "epsilon.delta").read_bytes(), source, payload)
        (work / f"utf8-{selected}-request.bin").write_bytes(request)
        print(f"Omega UTF-8 batch {selected}: {len(source)} Epsilon bytes; {len(payload)} input bytes; allowance {timeout}s", flush=True)
        started = time.monotonic()
        status, observation = execute(work / "epsilon", request, timeout)
        (work / f"utf8-{selected}-observation.bin").write_bytes(observation)
        assert status == 0 and observation[:5] == bytes(5), (selected, status, observation[:80])
        offset = 5
        first = sum(map(len, expected_batches[:selected]))
        # -4 is Alpha's Linux SIGILL Trap, not an authored process exit.
        # Wire marker zero requires independent observation of the signal.
        for index, expected in enumerate(expected_batches[selected], first):
            assert offset + 5 <= len(observation), ("truncated artifact frame", index)
            code = observation[offset]
            length = struct.unpack_from("<I", observation, offset + 1)[0]
            offset += 5
            assert code == max(expected, 0) and 0 < length <= 16777212
            assert offset + length <= len(observation), ("truncated artifact", index)
            tape = work / f"utf8-{index}.tape"
            executable = work / f"utf8-{index}"
            tape.write_bytes(observation[offset:offset + length])
            offset += length
            stamp(tape, executable)
            assert execute(executable) == (expected, b""), index
        assert offset == len(observation), "unexpected publication after the final artifact"
        print(f"Omega UTF-8 batch {selected}: {len(expected_batches[selected])} executables passed; {time.monotonic()-started:.3f}s", flush=True)
    if batch is None:
        print("Omega UTF-8: forty-five executables, 28 decoder boundaries, six arithmetic traps, two explicit traps, sixty-five refusal/reset checks passed in eleven fresh processes", flush=True)


def reconstruct(work):
    shell('''
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
