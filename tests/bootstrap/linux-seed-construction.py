"""Exercise the offline Linux launcher with an isolated source tree and an empty PATH."""

import argparse
import hashlib
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
BOOTSTRAP = ROOT / "tests/bootstrap/linux_x86_64/bootstrap"
BETA = Path("bootstrap/beta/beta_compiler.beta")
GAMMA = Path("bootstrap/gamma/interpreter/gamma_evaluator.beta")
COMPILER = [Path("bootstrap/gamma/compiler/compiler.gamma"),
            Path("bootstrap/gamma/compiler/frontend.gamma")]
NATIVE = Path("bootstrap/gamma/compiler/linux_x86_64/native.gamma")
ALPHA = Path("bootstrap/gamma/compiler/alpha/alpha_backend.gamma")

DELTA = Path("bootstrap/delta/compiler")
ENTRY = DELTA / "delta_compiler.gamma"
MANIFEST = DELTA / "implementation/implementation.gamma.sources"


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


def check_pair_lifetimes(work, compiler):
    # Retain both edges of a shared tree across several young-area refills.
    # The leaf also contains an integer numerically equal to a pair address.
    source = b"""(def share ((depth Int) (node Int)) Int
  (if depth (share (- depth 1) (pair node node)) node))
(def leaf ((depth Int) (node Int)) Int
  (if depth (leaf (- depth 1) (if (% depth 2) (first node) (second node))) node))
(def churn ((count Int) (held Int)) Int
  (if count
    (let temporary Int (pair count held)
      (churn (- (first temporary) 1) (second temporary)))
    held))
(def main () Int
  (let ((tree Int (share 32 (pair (read 0) 805306368)))
        (both Int (pair tree (churn 800000 tree)))
        (left Int (leaf 32 (first both)))
        (right Int (leaf 32 (second both))))
    (if (eq (second left) 805306368) (first right) 0)))
"""
    made = invoke([compiler], source)
    assert (made.returncode, made.stderr) == (0, b""), made
    program = work / "retained-pairs"
    program.write_bytes(made.stdout)
    program.chmod(0o700)
    result = invoke([program], b"*")
    assert (result.returncode, result.stdout, result.stderr) == (0, b"*", b""), result


def check_pair_capacity(work, compiler, compiler_source):
    # Four pairs per young area, sixteen allocations in total. Hold the entire
    # list live so every collection must preserve all of its previous nodes.
    for original, reduced in (
        (b"(def n_young_size () Int 8388608)", b"(def n_young_size () Int 128)"),
        (b"(def e_pair_heap_end () Int 137438953472)", b"(def e_pair_heap_end () Int 805306880)"),
    ):
        assert compiler_source.count(original) == 1
        compiler_source = compiler_source.replace(original, reduced)
    small = work / "small-heap-compiler"
    guest = work / "small-heap-program"
    program = b"""(def $application () Int 1)
(def fill ((n Int) (tail Int)) Int (if n (fill (- n 1) (pair n tail)) tail))
(def sum ((n Int) (p Int)) Int (if n (+ (first p) (sum (- n 1) (second p))) 0))
(def main () Int
  (let ((n Int (read 0)) (out Int (write (sum n (fill n 0))))) (pair 0 1)))
"""
    for producer, source, destination in ((compiler, compiler_source, small), (small, program, guest)):
        made = invoke([producer], source)
        assert made.returncode == 0 and not made.stderr, made
        destination.write_bytes(made.stdout)
        destination.chmod(0o700)
    # Publishing the application status requires one additional pair.
    for n in (0, 4, 5, 14, 15, 16, 17):
        result = invoke([guest], bytes([n]))
        expected = (0, bytes([n * (n + 1) // 2]), b"") if n < 16 else (252, b"", b"")
        assert (result.returncode, result.stdout, result.stderr) == expected, (n, result)


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
    seed = (ROOT / "bootstrap/alpha/alpha_x64_linux").read_bytes()
    for executable, tape in (("beta-seed", "admitted-beta.tape"), ("beta", "beta.tape"),
                             ("gamma-evaluator", "gamma.tape"), ("gamma0", "gamma0.tape"),
                             ("gamma1", "gamma1.tape"), ("gamma-compiler", "gamma2.tape"),
                             ("program", "program.tape"), ("successor", "successor.tape")):
        check_stamped(seed, out / executable, (out / tape).read_bytes())
    first = (out / "gamma0.tape").read_bytes()
    assert first == (out / "gamma1.tape").read_bytes() == (out / "gamma2.tape").read_bytes()


def checked_members(manifest):
    header, *rows = (ROOT / manifest).read_text().splitlines()
    assert header == "GammaSourceClosureV1" and rows
    for row in rows:
        _, _, size, digest, spelling = row.split(" ")
        path = manifest.parent / spelling
        data = (ROOT / path).read_bytes()
        assert len(data) == int(size) and hashlib.sha256(data).hexdigest() == digest
        yield path, data


def check_manifest_edits(work):
    # A source root with no lower stages stops immediately after taking the
    # Delta snapshot. Changes to the manifest must change that snapshot.
    source = work / "edited"
    members = source / MANIFEST.parent
    members.mkdir(parents=True)
    (source / ENTRY).write_bytes(b"; prefix")
    (members / "z.gamma").write_bytes(b" Z")
    (members / "a.gamma").write_bytes(b" A")
    (members / "link.gamma").symlink_to("a.gamma")
    (members / "linked").symlink_to(members, target_is_directory=True)
    (members / "binary.gamma").write_bytes(b"\x80a")
    header = "GammaSourceClosureV1"
    first = f"member {'1' * 64} 2 {hashlib.sha256(b' Z').hexdigest()} z.gamma"
    second = f"member {'2' * 64} 2 {hashlib.sha256(b' A').hexdigest()} a.gamma"

    def build(name, text):
        (source / MANIFEST).write_bytes(text.encode("ascii"))
        out = work / name
        result = invoke([BOOTSTRAP, "--delta", source, out])
        assert result.returncode and not result.stdout, result
        return out, result.stderr

    out, error = build("edited-output", "\r\n".join((header, first, second)))
    assert b"beta_compiler.beta" in error
    assert (out / "program.gamma").read_bytes() == b"; prefix Z A"
    before = (out / "program.gamma").read_bytes()
    again = invoke([BOOTSTRAP, "--delta", source, out])
    assert again.returncode and (out / "program.gamma").read_bytes() == before

    cases = [
        "bad-header\n" + first, header, header + "\n\n" + first,
        header + "\n" + first.replace(" 2 ", " 1 "),
        header + "\n" + first.replace(" 2 ", " 3 "),
        header + "\n" + first.replace(" 2 ", " 02 "),
        header + "\n" + first.replace(" 2 ", " 18446744073709551616 "),
        header + "\n" + first.replace("member ", "member  "),
        header + "\n" + first.replace(hashlib.sha256(b" Z").hexdigest(), "bad"),
        "\n".join((header, second, first)), "\n".join((header, first, first)),
        "\n".join((header, first, second.replace("a.gamma", "z.gamma"))),
    ]
    for path in ("missing.gamma", "../a.gamma", "/a.gamma", "./a.gamma",
                 "foo//a.gamma", "bad name.gamma", ".gamma", "a.delta", "link.gamma",
                 "linked/a.gamma", "binary.gamma"):
        cases.append(header + "\n" + first.replace("z.gamma", path))
    for i, text in enumerate(cases):
        out, error = build(f"refused-{i}", text)
        assert b"Delta compiler source" in error and not (out / "program.gamma").exists(), (text, error)


def check_delta(work, source, alpha):
    check_manifest_edits(work)
    expected = bytearray((ROOT / ENTRY).read_bytes())
    for path, data in checked_members(MANIFEST):
        (source / path).parent.mkdir(parents=True, exist_ok=True)
        (source / path).write_bytes(data)
        expected.extend(data)
    for path in (MANIFEST, ENTRY):
        shutil.copyfile(ROOT / path, source / path)
    support = b"".join(data for _, data in checked_members(DELTA / "support/support.gamma.sources"))
    customer = b"(def main ((source Bytes)) Bytes (bytes_concat source source))\n"
    request = work / "double.dcreq"
    request.write_bytes(b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(customer)) + customer + support)
    out = work / "delta"
    command = [BOOTSTRAP, "--alpha"] if alpha else [BOOTSTRAP]
    result = invoke([*command, "--delta", source, out, request])
    assert (result.returncode, result.stdout, result.stderr) == (0, b"", b""), result
    assert (out / "program.gamma").read_bytes() == expected
    direct = invoke([out / "gamma-compiler"], expected)
    artifact = out / ("program.tape" if alpha else "program")
    assert (direct.returncode, direct.stdout, direct.stderr) == (0, artifact.read_bytes(), b""), direct
    if alpha:
        check_alpha_stages(out)
    for payload in (b"", b"A\x00B", bytes(range(256))):
        observed = invoke([out / "successor"], payload)
        assert (observed.returncode, observed.stdout, observed.stderr) == (0, payload * 2, b"")


def check(work, alpha, delta):
    source = work / "source"
    compiler_files = [*COMPILER, ALPHA if alpha else NATIVE]
    for name in (BETA, GAMMA, *compiler_files):
        dest = source / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, dest)

    if delta:
        check_delta(work, source, alpha)
        return
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
    seed = (ROOT / "bootstrap/alpha/alpha_x64_linux").read_bytes()
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
    check_pair_lifetimes(work, compiler)
    check_pair_capacity(work, compiler, (out / "compiler.gamma").read_bytes())
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
    parser.add_argument("--delta", action="store_true", help="build Delta from the canonical source tree")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as directory:
        check(Path(directory), args.alpha, args.delta)
    print("Cold Linux construction, changed programs and refusals passed")
