"""Exercise the cold Linux command with only its five source inputs available."""

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
            Path("bootstrap/compiled_vm/frontend.gamma"),
            Path("bootstrap/independent/native.gamma")]


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


def check(work):
    source = work / "source"
    for name in (BETA, GAMMA, *COMPILER):
        dest = source / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, dest)

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
        hole = struct.pack("<I", len(tape)) + tape
        expected = seed[:0x3000] + hole + seed[0x3000 + len(hole):]
        assert (out / name).read_bytes() == expected, name

    compiler_source = b"\n".join((source / name).read_bytes() for name in COMPILER)
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
    with tempfile.TemporaryDirectory() as directory:
        check(Path(directory))
    print("Cold Linux Gamma construction, changed programs and refusals passed")
