"""Exercise the cold Linux command with only its five source inputs available."""

import argparse
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
BOOTSTRAP = ROOT / "bootstrap/linux_x86_64/bootstrap"
BETA = Path("bootstrap/interpreted/1_beta/beta_compiler.beta")
GAMMA = Path("bootstrap/interpreted/2_gamma/gamma_evaluator.beta")
COMPILER = [Path("bootstrap/compiled_vm/compiler.gamma"),
            Path("bootstrap/compiled_vm/frontend.gamma")]
NATIVE = Path("bootstrap/independent/native.gamma")
ALPHA = Path("bootstrap/compiled_vm/alpha_backend.gamma")


def invoke(argv, data=b""):
    return subprocess.run(argv, input=data, capture_output=True, cwd="/", env={"PATH": ""})


def check_native(program):
    image = program.read_bytes()
    assert image[:7] == b"\x7fELF\x02\x01\x01"
    assert struct.unpack_from("<HH", image, 16) == (2, 62)
    offset, = struct.unpack_from("<Q", image, 32)
    stride, count = struct.unpack_from("<HH", image, 54)
    segments = [struct.unpack_from("<I", image, offset + i * stride)[0]
                for i in range(count)]
    assert 1 in segments and 2 not in segments and 3 not in segments


def check_source_producer(work, source, compiler=None, *, alpha=False):
    command = [BOOTSTRAP, "--alpha"] if alpha else [BOOTSTRAP]
    producer = work / "producer.gamma"
    producer.write_bytes(b"""(def $application () Int 1)
(def emit_after_header ((i Int)) Int
  (if (eq i (input)) (pair 0 1)
    (let byte Int (write (read i)) (emit_after_header (+ i 1)))))
(def main () Int (emit_after_header 1))
""")
    request = work / "producer input"
    program = b"(def main () Int (+ (read 0) 19))"
    request.write_bytes(b"\xff" + program)
    out = work / "from-producer"
    result = invoke([*command, source, out, producer, request])
    assert (result.returncode, result.stdout, result.stderr) == (0, b"", b""), result
    assert (out / "program.input").read_bytes() == request.read_bytes()
    assert (out / "producer.output").read_bytes() == program
    if alpha:
        check_alpha_stages(out)
        compiler = out / "gamma-compiler"
    direct_artifact = out / ("successor.tape" if alpha else "successor")
    direct = invoke([compiler], program)
    assert (direct.returncode, direct.stdout, direct.stderr) == (
        0, direct_artifact.read_bytes(), b""), direct
    check_native(out / "successor")
    for byte in (0, 23):
        result = invoke([out / "successor"], bytes([byte]))
        assert (result.returncode, result.stdout, result.stderr) == (
            0, bytes([byte + 19]), b""), result

    rejected = work / "bad-producer-output"
    invalid = b"(def main () Int undefined)"
    request.write_bytes(b"\x00" + invalid)
    result = invoke([*command, source, rejected, producer, request])
    assert result.returncode == 249 and not result.stdout, result
    assert (rejected / "producer.output").read_bytes() == invalid
    assert not (rejected / "successor").exists() and not (rejected / "successor.tape").exists()
    missing = work / "missing-producer-input"
    result = invoke([*command, source, missing, producer, work / "absent.input"])
    assert result.returncode and not missing.exists()


def check_stamped(seed, executable, tape):
    field = struct.pack("<I", len(tape)) + tape
    expected = seed[:0x3000] + field + seed[0x3000 + len(field):]
    assert executable.read_bytes() == expected, executable


def check_alpha_stages(out):
    seed = (ROOT / "bootstrap/interpreted/0_alpha/alpha_x64_linux").read_bytes()
    for executable, tape in (("beta-seed", "admitted-beta.tape"), ("beta", "beta.tape"),
                             ("gamma-evaluator", "gamma.tape"), ("gamma0", "gamma0.tape"),
                             ("gamma1", "gamma1.tape"), ("gamma-compiler", "gamma2.tape"),
                             ("program", "program.tape"), ("successor", "successor.tape")):
        check_stamped(seed, out / executable, (out / tape).read_bytes())
    first = (out / "gamma0.tape").read_bytes()
    assert first == (out / "gamma1.tape").read_bytes() == (out / "gamma2.tape").read_bytes()


def check(work, alpha):
    source = work / "source"
    compiler_files = [*COMPILER, ALPHA if alpha else NATIVE]
    for name in (BETA, GAMMA, *compiler_files):
        dest = source / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, dest)

    if alpha:
        check_source_producer(work, source, alpha=True)
        return
    out = work / "built"
    caller_source = work / "customer input.gamma"
    caller_source.write_bytes(b"(def main () Int (+ (read 0) 7))")
    run = invoke([BOOTSTRAP, source, out, caller_source])
    assert (run.returncode, run.stdout, run.stderr) == (0, b"", b""), run
    beta = (ROOT / BETA.with_name("beta_compiler_bytecode.tape")).read_bytes()
    gamma = (ROOT / GAMMA.with_name("gamma_evaluator_bytecode.tape")).read_bytes()
    assert (out / "beta.tape").read_bytes() == beta
    assert (out / "gamma.tape").read_bytes() == gamma
    seed = (ROOT / "bootstrap/interpreted/0_alpha/alpha_x64_linux").read_bytes()
    for name, tape in [("beta-seed", beta), ("beta", beta), ("gamma-evaluator", gamma)]:
        check_stamped(seed, out / name, tape)

    compiler_source = b"\n".join((source / name).read_bytes() for name in compiler_files)
    assert (out / "compiler.gamma").read_bytes() == compiler_source
    assert (out / "compiler.request").read_bytes() == (
        struct.pack("<I", len(compiler_source)) + compiler_source + compiler_source)
    compiler = out / "gamma-compiler"
    image = compiler.read_bytes()
    assert (out / "gamma0").read_bytes() == (out / "gamma1").read_bytes() == image
    assert not (out / "stage.partial").exists()
    check_native(BOOTSTRAP)
    check_native(compiler)
    check_native(out / "program")
    assert (out / "program.gamma").read_bytes() == caller_source.read_bytes()
    direct = invoke([compiler], caller_source.read_bytes())
    assert (direct.returncode, direct.stdout, direct.stderr) == (
        0, (out / "program").read_bytes(), b""), direct
    result = invoke([out / "program"], bytes([35]))
    assert (result.returncode, result.stdout, result.stderr) == (0, b"*", b""), result
    for increment in (7, 19):
        source_text = f"(def main () Int (+ (read 0) {increment}))".encode()
        made = invoke([compiler], source_text)
        assert made.returncode == 0 and not made.stderr, made
        customer = work / "customer"
        customer.write_bytes(made.stdout)
        customer.chmod(0o700)
        check_native(customer)
        for byte in (1, 38):
            result = invoke([customer], bytes([byte]))
            assert (result.returncode, result.stdout, result.stderr) == (
                0, bytes([byte + increment]), b""), result
    invalid = invoke([compiler], b"(def main () Int undefined)")
    assert (invalid.returncode, invalid.stdout) == (249, b""), invalid
    check_source_producer(work, source, compiler)

    bad_customer = work / "invalid-customer"
    caller_source.write_bytes(b"(def main () Int undefined)")
    invalid_stage = invoke([BOOTSTRAP, source, bad_customer, caller_source])
    assert (invalid_stage.returncode, invalid_stage.stdout) == (249, b""), invalid_stage
    assert (bad_customer / "gamma-compiler").read_bytes() == image
    assert not (bad_customer / "program").exists()

    missing_out = work / "missing-input"
    missing = invoke([BOOTSTRAP, source, missing_out, work / "absent.gamma"])
    assert missing.returncode and not missing_out.exists()
    again = invoke([BOOTSTRAP, source, out])
    assert again.returncode and compiler.read_bytes() == image
    for label, changed, code in [("different", b"halt r0\n", 1),
                                 ("malformed", b"halt r0\nunknown\n", 8)]:
        (source / BETA).write_bytes(changed)
        rejected = work / label
        result = invoke([BOOTSTRAP, source, rejected])
        assert result.returncode == code and not result.stdout, result
        assert not (rejected / "beta.tape").exists()
        assert not (rejected / "gamma-evaluator").exists()
        assert not (rejected / "gamma-compiler").exists()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--alpha", action="store_true", help="exercise the Alpha tape target")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as directory:
        check(Path(directory), args.alpha)
    print("Cold Linux Gamma construction, changed programs and refusals passed")
