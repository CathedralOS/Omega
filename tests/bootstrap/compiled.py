"""Shared construction and comparison helpers for the compiled bootstrap routes."""
import argparse
import hashlib
import os
from pathlib import Path
import signal
import struct
import subprocess
import time

from conformance.vectors import GAMMA_CASES, INVALID_GAMMA, epsilon_observations, epsilon_request, ocreq_request

ROOT = Path(__file__).resolve().parents[2]


class Invalid(ValueError):
    """A source-constructed compiler rejected the supplied program."""


def add_invocation_budget(parser, option="--seconds"):
    def positive_seconds(value):
        seconds = int(value)
        if seconds <= 0:
            raise argparse.ArgumentTypeError(f"{option} must be positive")
        return seconds

    parser.add_argument(option, type=positive_seconds,
                        help="optional wall limit per invocation (default: unlimited)")


def invoke(command, data, seconds, *, cwd=None, env=None):
    # Cold construction forks producers. Stop the whole invocation on timeout.
    with subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, start_new_session=True) as proc:
        try:
            stdout, stderr = proc.communicate(data, timeout=seconds)
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            proc.communicate()
            raise
    if proc.returncode or stderr:
        raise RuntimeError(f"exit {proc.returncode}, stdout {stdout[:80]!r}, stderr {stderr[:200]!r}")
    return stdout


def shell(script, *args, seconds=None):
    command = ["sh", "-eu", "-c", script, "compiled-vm", *map(str, args)]
    env = {**os.environ, "OMEGA_REPO_ROOT": str(ROOT)}
    if seconds is None:
        subprocess.run(command, cwd=ROOT, env=env, check=True)
    else:
        invoke(command, b"", seconds, cwd=ROOT, env=env)


def stamp(tape, executable):
    shell('. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"; '
          '. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"; '
          'stamp_seed "$1" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$2"', tape, executable)


def execute(executable, payload=b"", timeout=None):
    result = subprocess.run([str(executable)], input=payload, capture_output=True,
                            timeout=timeout)
    if result.stderr:
        raise AssertionError(result.stderr)
    return result.returncode, result.stdout


def gamma_request(source, guest=b""):
    """Frame a Gamma source and its sealed guest input for the interpreter."""
    return struct.pack("<I", len(source)) + source + guest


def reference(evaluator, source, payload=b"", timeout=None):
    return execute(evaluator, gamma_request(source, payload), timeout)


def gamma_backend_source(target):
    """Read the complete Gamma compiler source for the requested target."""
    root = ROOT / "bootstrap/gamma/compiler"
    if target == "alpha":
        backend = "alpha/alpha_backend.gamma"
    elif target == "linux":
        backend = "linux_x86_64/native.gamma"
    else:
        raise ValueError(f"unknown Gamma target {target!r}")
    return b"\n".join((root / name).read_bytes() for name in ("compiler.gamma", "frontend.gamma", backend))


def produce(compiler, source, artifact, seconds):
    start = time.monotonic()
    status, data = execute(compiler, source, seconds)
    if status != 0 or not data:
        raise RuntimeError(f"{artifact.name}: compiler exit {status}, {len(data)} output bytes")
    artifact.write_bytes(data)
    print(f"{artifact.name}: {len(data)} bytes, sha256={hashlib.sha256(data).hexdigest()}, "
          f"{time.monotonic() - start:.2f}s", flush=True)
    return data


def executable(artifact, target):
    """Make a produced Linux ELF or Alpha tape runnable and return its path."""
    if target == "linux":
        artifact.chmod(0o700)
        return artifact
    if target == "alpha":
        program = artifact.with_suffix(".vm")
        stamp(artifact, program)
        return program
    raise ValueError(f"unknown Gamma target {target!r}")


def gamma_program(host, runner, source, artifact, seconds):
    """Return the runner and input prefix for this Gamma source on the given host.

    For the interpreted host, runner is the Gamma evaluator. For a compiled
    host, runner is a Gamma compiler that produces code for that host.
    """
    if host == "gamma":
        return runner, gamma_request(source)
    if host not in ("alpha", "linux"):
        raise ValueError(f"unknown Gamma host {host!r}")
    produce(runner, source, artifact, seconds)
    return executable(artifact, host), b""


def compile_program(work, source, name="subject", *, compiler, realize=stamp):
    tape = work / (name + (".tape" if realize is stamp else ".elf"))
    tape.write_bytes(compiler(source).compile())
    executable = work / name
    realize(tape, executable)
    return executable


def call_depth_controls(work, compiler, seconds, realize=stamp):
    # Main and the initial dive occupy two of the 131,072 Gamma frames.
    # The last admitted frame uses both projections and nested pair allocation.
    depth_source = b"""
(def $application () Int 1)
(def dive ((n Int)) Int
  (if n (+ 0 (dive (- n 1))) (second (pair 7 (first (pair (* 6 7) 7))))))
(def main () Int
  (let ((prefix Int (write 88)) (result Int (dive (+ 131070 (read 0))))
        (sent Int (write result))) (pair 0 1)))
"""
    depth = compile_program(work, depth_source, "call-depth", compiler=compiler, realize=realize)
    assert execute(depth, b"\x00", seconds) == (0, b"X*")
    assert execute(depth, b"\x01", seconds) == (250, b"")


def controls(work, evaluator, compiler, realize=stamp, invalid=Invalid, *, seconds=None):
    names = """(def main () Int (prefix_z 40))
(def prefix_zz ((n Int)) Int (prefix_y n))
(def prefix_y ((n Int)) Int (+ n 2))
(def prefix_z ((n Int)) Int (prefix_zz n))"""
    lookalikes = """(def firsT ((p Int)) Int (second p))
(def $applicatiox ((n Int)) Int (+ n 1))
(def main () Int (firsT (pair 0 ($applicatiox 41))))"""
    # The two tagged slots are 256 bytes apart, so truncating a slot offset
    # to one byte overwrites the first pair when storing the last.
    padding = " ".join(f"(padding{i} Int 0)" for i in range(15))
    distant_pair = f"""(def main () Int
      (let ((low Int (pair 40 3)) {padding} (high Int (pair 1 2)))
        (+ (first low) (second high))))"""
    cases = [*GAMMA_CASES, ("tagged frame slots 256 bytes apart", distant_pair, b"", (0, b"*")),
             ("forward calls with common prefixes", names, b"", (0, b"*")),
             ("names differing at the last byte of a keyword", lookalikes, b"", (0, b"*"))]
    for label, text, payload, expected in cases:
        source = text.encode("ascii")
        actual = execute(compile_program(work, source, compiler=compiler, realize=realize), payload, seconds)
        original = reference(evaluator, source, payload, seconds)
        assert actual == original == expected, (label, actual, original, expected)
    duplicate = names.encode("ascii") + b" (def prefix_y () Int 0)"
    invalid_sources = [*INVALID_GAMMA, duplicate]
    for source in invalid_sources:
        try:
            compiler(source).compile()
        except invalid:
            pass
        else:
            raise AssertionError(("invalid source compiled", source))
        assert reference(evaluator, source, timeout=seconds) == (1, b""), source
    print(f"controls: {len(cases)} exact differential observations; {len(invalid_sources)} rejected sources", flush=True)
    return len(cases), len(invalid_sources)


def pack(manifest, output, prefix=None):
    binary = os.environ.get("OMEGA_XTASK_BIN") or str(ROOT / "target" / "debug" / "xtask")
    if not os.access(binary, os.X_OK):
        raise SystemExit(f"bootstrap artifact: xtask binary is not executable at {binary}")
    command = [binary, "source-closure", str(ROOT / manifest), str(output)]
    if prefix:
        command.extend(["--prefix", str(ROOT / prefix)])
    subprocess.run(command, check=True)
    return output.read_bytes()


def delta_request(subject, support, profile=1):
    return b"DCREQ\x01\x00\x00" + struct.pack("<II", profile, len(subject)) + subject + support


def pack_delta_compiler(work):
    return pack("bootstrap/delta/compiler/implementation/implementation.gamma.sources",
                work / "delta.gamma", "bootstrap/delta/compiler/delta_compiler.gamma")


def customer(work, evaluator, producer, realize=stamp, *, seconds=None):
    source = pack_delta_compiler(work)
    support = pack("bootstrap/delta/compiler/support/support.gamma.sources", work / "support.bin")
    started = time.monotonic()
    compiler = compile_program(work, source, "delta", compiler=producer, realize=realize)
    artifact = work / ("delta.tape" if realize is stamp else "delta.elf")
    print(f"Delta customer: {len(source)} source bytes -> {artifact.stat().st_size} artifact bytes; compilation {time.monotonic()-started:.3f}s", flush=True)
    delta = b"(def main ((source Bytes)) Bytes (bytes_concat source source))\n"
    request = delta_request(delta, support)
    (work / "request.bin").write_bytes(request)
    started = time.monotonic()
    actual = execute(compiler, request, seconds)
    elapsed = time.monotonic() - started
    original = reference(evaluator, source, request, seconds)
    assert actual == original and actual[0] == 0, (actual[0], original[0], len(actual[1]), len(original[1]))
    receipt = actual[1]
    (work / "receipt.gamma").write_bytes(receipt)
    result = compile_program(work, receipt, "receipt", compiler=producer, realize=realize)
    for payload in (b"", b"ABC", bytes(range(256))):
        assert execute(result, payload, seconds) == reference(evaluator, receipt, payload, seconds) == (0, payload * 2)
    for request in (b"", b"DCREQ\x01\x00\x00" + struct.pack("<II", 0, 0)):
        assert execute(compiler, request, seconds) == reference(evaluator, source, request, seconds)
    print(f"Delta customer: exact receipt {len(receipt)} bytes sha256={hashlib.sha256(receipt).hexdigest()}; compiled execution {elapsed:.3f}s (functional run, not a comparative benchmark)", flush=True)


def epsilon_customer(work, producer, realize=stamp, *, seconds=None):
    closure = pack("bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources", work / "epsilon.delta")
    subject = closure + (ROOT / "bootstrap/epsilon/interpreter.delta").read_bytes()
    request = delta_request(subject, (work / "support.bin").read_bytes())
    (work / "epsilon-request.bin").write_bytes(request)
    started = time.monotonic()
    status, receipt = execute(work / "delta", request, seconds)
    assert status == 0, (status, receipt[:40])
    elapsed = time.monotonic() - started
    (work / "epsilon.gamma").write_bytes(receipt)
    shell('. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"; '
          '. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"; '
          'require_epsilon_evaluator_entry_receipt_identity "$1"', work / "epsilon.gamma")
    executable = compile_program(work, receipt, "epsilon", compiler=producer, realize=realize)
    for label, request, expected in epsilon_observations(closure):
        actual = execute(executable, request, seconds)
        assert actual == expected, (label, actual, expected)
    artifact = work / ("epsilon.tape" if realize is stamp else "epsilon.elf")
    print(f"Epsilon customer: reconstructed bound {len(receipt)}-byte receipt in {elapsed:.3f}s; {artifact.stat().st_size} artifact bytes; exit, stdin, trap, reject and malformed-envelope observations passed", flush=True)


def omega_customer(work, timeout, controls=False, *, observe=None):
    closure = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/conformance/omega"
    source = closure + (gate / ("controls_b.epsilon" if controls else "main_ocreq.epsilon")).read_bytes()
    stdin = b"" if controls else ocreq_request((gate / "program.omg").read_bytes(), b"program.omg")
    request = epsilon_request((work / "epsilon.delta").read_bytes(), source, stdin)
    label = "omega-controls" if controls else "omega"
    (work / (label + "-request.bin")).write_bytes(request)
    print(f"{label} customer: {len(source)} Epsilon source bytes", flush=True)
    started = time.monotonic()
    if observe is None:
        status, observation = execute(work / "epsilon", request, timeout)
    else:
        status, observation = observe(request, timeout)
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
    assert execute(work / (label + "-program"), timeout=timeout) == (expected, b"")
    print(f"{label} customer: {len(tape)} Alpha bytes -> exit {expected}; elapsed {time.monotonic()-started:.3f}s", flush=True)


def omega_values_customer(work, timeout):
    """Execute the source-owned parameter/local/call checks and their tapes."""
    closure = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    entry = (ROOT / "tests/bootstrap/conformance/omega/controls_values.epsilon").read_bytes()
    source = closure + entry
    request = epsilon_request((work / "epsilon.delta").read_bytes(), source, b"")
    (work / "values-source.epsilon").write_bytes(source)
    (work / "values-request.bin").write_bytes(request)
    print(f"Omega values customer: {len(source)} source bytes", flush=True)
    started = time.monotonic()
    status, observation = execute(work / "epsilon", request, timeout)
    (work / "values-observation.bin").write_bytes(observation)
    assert status == 0 and observation[:5] == bytes(5), (status, observation[:80])
    offset = 5
    for index, expected in enumerate((42, 42, 53, 44, 52, 42, 56, 37, 0, 2, 47, 42, 43)):
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
        assert execute(executable, timeout=timeout) == (expected, b""), index
    assert offset == len(observation), "unexpected publication after the final artifact"
    print(f"Omega values customer: thirteen emitted executables and 34 source-owned refusal/reset checks passed; {time.monotonic()-started:.3f}s", flush=True)


def omega_control_customer(work, timeout):
    """Compile the repository lexer predicates and execute scalar controls."""
    closure = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/conformance/omega"
    frontend = ROOT / "source/subsystems/compiler/source_to_psi"
    stage = frontend / "pipeline/source_files_to_tokens"
    lexer = (stage / "lexer.omg").read_bytes()
    digits = (stage / "digits.omg").read_bytes()
    declarations = []
    for unit, name in ((lexer, b"is_identifier_start"), (lexer, b"is_identifier_continue"),
                       (digits, b"lexer_digit_in_base"), (digits, b"lexer_digit_value")):
        start = unit.index(b"machine " + name + b"(")
        end = unit.index(b"\n}", start) + 2
        declarations.append(unit[start:end])
    tokens = (frontend / "representations/tokens/numeric.omg").read_bytes()
    start = tokens.index(b"public data NumericBase ")
    end = tokens.index(b"\n}", start) + 2
    declarations.insert(0, tokens[start:end])
    program = b"\n".join(declarations) + b"\n" + (gate / "lexer_controls.omg").read_bytes()
    source = closure + (gate / "controls_control.epsilon").read_bytes()
    request = epsilon_request((work / "epsilon.delta").read_bytes(), source, program)
    (work / "control-source.epsilon").write_bytes(source)
    (work / "control-program.omg").write_bytes(program)
    (work / "control-request.bin").write_bytes(request)
    print(f"Omega runtime control: {len(source)} Epsilon bytes; {len(program)} Omega bytes", flush=True)
    started = time.monotonic()
    status, observation = execute(work / "epsilon", request, timeout)
    (work / "control-observation.bin").write_bytes(observation)
    assert status == 0 and observation[:5] == bytes(5), (status, observation[:80])
    offset = 5
    for index, expected in enumerate((42, 42, 43, 44, 45, 46, 42, 47, 42, 43, 43, 43, 42, 43, 42, 43, 42, 42, 0, 42, 42, 42, 0, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42, 42)):
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
        assert execute(executable, timeout=timeout) == (expected, b""), index
    assert offset == len(observation), "unexpected publication after the final artifact"
    print(f"Omega runtime control: 39 emitted executables, actual lexer byte/base matrix, nominal sums, terminal values, lexical machine scopes, source-set lookup/reset, chained casts and refusal checks passed; {time.monotonic()-started:.3f}s", flush=True)


def omega_storage_program():
    gate = ROOT / "tests/bootstrap/conformance/omega"
    source_files = ROOT / "source/subsystems/compiler/source_to_psi/representations/source_files/source.omg"
    construction = (ROOT / "source/application/construction/main.omg").read_bytes()
    start = construction.index(b"data ConstructionInput {")
    end = construction.index(b"\n}", start) + 2
    return b"\n\n".join((
        source_files.read_bytes(),
        construction[start:end],
        (gate / "qualified_array_elements.omg").read_bytes(),
        (gate / "indexed_record_fields.omg").read_bytes(),
        (gate / "storage_controls.omg").read_bytes(),
    ))


def omega_storage_customer(work, timeout):
    """Compile the product storage declarations and exercise indexed reads and writes."""
    closure = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
    gate = ROOT / "tests/bootstrap/conformance/omega"
    program = omega_storage_program()
    source = closure + (gate / "controls_storage.epsilon").read_bytes()
    request = epsilon_request((work / "epsilon.delta").read_bytes(), source, program)
    (work / "storage-source.epsilon").write_bytes(source)
    (work / "storage-program.omg").write_bytes(program)
    (work / "storage-request.bin").write_bytes(request)
    print(f"Omega storage: {len(source)} Epsilon bytes; {len(program)} Omega bytes", flush=True)
    started = time.monotonic()
    status, observation = execute(work / "epsilon", request, timeout)
    (work / "storage-observation.bin").write_bytes(observation)
    assert status == 0 and observation[:5] == bytes(5), (status, observation[:80])
    offset = 5
    for index, expected in enumerate((42, 43, 44, 45, 46, 0, 2, 48, 44, 0, 51, 53, 42, 42, 42, 42, 59)):
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
        assert execute(executable, timeout=timeout) == (expected, b""), index
    assert offset == len(observation), "unexpected publication after the final artifact"
    print(f"Omega storage: seventeen executables, SourceUnit append/read, ModulePath boundaries, indexed SourceRange fields, u64 arrays and all refusal/reset checks passed; {time.monotonic()-started:.3f}s", flush=True)


def omega_utf8_customer(work, timeout, batch=None):
    """Compile the real UTF-8 decoder and execute arithmetic/trap boundaries."""
    closure = pack("bootstrap/omega/compiler/omega_compiler.epsilon.sources", work / "bootstrap-compiler.epsilon")
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
    # Batch 13 exercises the hex conversion used by the product lexer.
    digits = (ROOT / "source/subsystems/compiler/source_to_psi/pipeline/source_files_to_tokens/digits.omg").read_bytes()
    start = digits.index(b"machine hex_digit_value(")
    end = digits.index(b"\n}", start) + 2
    loans = (ROOT / "source/subsystems/compiler/source_to_psi/pipeline/typed_trees_to_checked_trees/loans.omg").read_bytes()
    entry = loans.index(b"public machine holds_references(")
    traversal = loans.index(b"machine walk_reference_types(", entry)
    end_traversal = loans.index(b"\n}", traversal) + 2
    programs = {
        0: program,
        13: digits[start:end] + b"\n" + (gate / "hex_digit_controls.omg").read_bytes(),
        15: (gate / "fixed_array_loans.omg").read_bytes(),
        # Batch 16 assigns the test file this logical path before compiling it.
        16: b"main.omg" + (gate / "loan_traversal_setup.omg").read_bytes() + b"\n" + loans[entry:end_traversal],
    }
    # Each refusal batch runs positives before and after the refusals on the same compiler.
    expected_batches = (
        (42,),
        (42, -4, -4, -4, 42, 42, 42, 42, -4),
        (42, -4, -4),
        (42, -4, 59),
        (42, 43, 44, 59),
        (42, 42, 43, 44, 43, 45, 59),
        (42, 42, 42, 42),
        (42, -4, 43, 43, 44),
        (42, 44),
        (42, 42, 43, 44, 45),
        (42, -4, 42, 42, 42, 42),
        (42, 42, 42, 42, 42, 42, -4, 42, 42),
        (42, 42, 42, 42, 42),
        (42, -4, 42),
        (42, 42, 42, 42, 42, 42),
        (42, 42, 42, 42, 42),
        (42, -4),
    )
    batches = range(len(expected_batches)) if batch is None else (batch,)
    for selected in batches:
        assert 0 <= selected < len(expected_batches), selected
        payload = bytes([selected]) + programs.get(selected, b"")
        request = epsilon_request((work / "epsilon.delta").read_bytes(), source, payload)
        (work / f"utf8-{selected}-request.bin").write_bytes(request)
        print(f"Omega UTF-8 batch {selected}: {len(source)} Epsilon bytes; {len(payload)} input bytes", flush=True)
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
            assert execute(executable, timeout=timeout) == (expected, b""), index
        assert offset == len(observation), "unexpected publication after the final artifact"
        print(f"Omega UTF-8 batch {selected}: {len(expected_batches[selected])} executables passed; {time.monotonic()-started:.3f}s", flush=True)
    if batch is None:
        print("Omega UTF-8: fifty-nine executables, 28 decoder boundaries, seven arithmetic traps, three explicit traps, eighty-nine refusal/reset checks passed in fourteen fresh processes", flush=True)


def reconstruct(work, *, seconds=None):
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
    ''', work, seconds=seconds)
    print("Beta self-reconstruction and Gamma source construction: byte-exact", flush=True)
