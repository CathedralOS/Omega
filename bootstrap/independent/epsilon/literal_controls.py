"""Exercise Delta byte literals through the actual Epsilon source encoder."""
import json
from pathlib import Path
import struct


def run(work, vm, producer, realize, seconds):
    support = (work / "support.bin").read_bytes()

    def compile_delta(source, profile=1):
        request = b"DCREQ\x01\x00\x00" + struct.pack("<II", profile, len(source)) + source + support
        return vm.execute(work / "delta", request, seconds)

    positives = [
        (b'""', b""),
        (b'"(def ; literal)"', b"(def ; literal)"),
        (b'"a\\n\\r\\t\\\\\\\"z"', b'a\n\r\t\\"z'),
        (b'"' + b"x" * 1024 + b'"', b"x" * 1024),
        (b'(bytes_concat "left" "right")', b"leftright"),
        (b'(let text Bytes "bound" text)', b"bound"),
    ]
    for index, (expression, expected) in enumerate(positives):
        source = b"(def main ((source Bytes)) Bytes " + expression + b")\n"
        status, receipt = compile_delta(source)
        assert status == 0, ("literal acceptance", index, status, receipt)
        executable = vm.compile_program(work, receipt, f"literal-{index}", compiler=producer,
                                        realize=realize)
        assert vm.execute(executable) == (0, expected), ("literal bytes", index)

    # The canonical request admits only profile one. Exercise the existing
    # raw-source diagnostic entry separately, preserving that boundary.
    development_source = vm.pack(
        "bootstrap/interpreted/3_delta/implementation/implementation.gamma.sources",
        work / "literal-development.gamma",
        "tests/delta/staged-compiler/development_driver.gamma")
    development = vm.compile_program(work, development_source, "literal-development",
                                     compiler=producer, realize=realize)
    source = b'(def text () Bytes "unused") (def main () Int 42)\n'
    status, receipt = vm.execute(development, source + support, seconds)
    assert status == 0, ("scalar profile literal runtime", status)
    executable = vm.compile_program(work, receipt, "literal-scalar", compiler=producer,
                                    realize=realize)
    assert vm.execute(executable) == (0, b"*")

    malformed = [b'"', b'"unterminated', b'"last\\', b'"escaped\\"',
                 b'"unknown\\q"', b'"zero\\0"', b'"hex\\x41"', b'"raw\nline"',
                 b'"raw\tbyte"', b'"closed"tail']
    for index, expression in enumerate(malformed):
        source = b"(def main ((source Bytes)) Bytes " + expression + b")\n"
        status, observation = compile_delta(source)
        expected = (b"\xffDCOUT\x01\x00", 1, 1, 0, 4, source.index(b'"'), 0, 0)
        assert status == 1 and len(observation) == 40, ("literal refusal", index, status)
        assert struct.unpack("<8sBBHIQQQ", observation) == expected, (index, observation)
    for source, code in [
            (b'(def main ((source Bytes)) Int "wrong")', 15),
            (b'(def "name" ((source Bytes)) Bytes source)', 4),
            (b'(def main ((source Bytes)) Bytes ("head" source))', 4)]:
        status, observation = compile_delta(source)
        assert status == 1 and len(observation) == 40
        fields = struct.unpack("<8sBBHIQQQ", observation)
        assert fields[4:6] == (code, source.index(b'"')), (source, fields)

    # This is production source for the next compiler, not a test-only printer.
    root = vm.ROOT
    base = vm.pack("bootstrap/interpreted/4_epsilon/epsilon_compiler.delta.sources",
                   work / "literal-emitter-base.delta")
    driver = root / "tests/delta/byte-literals"
    subject = base + b"\n" + (root / "bootstrap/independent/epsilon/emission.delta").read_bytes()
    subject += b"\n" + (driver / "emitter_driver.delta").read_bytes()
    (work / "literal-emitter.delta").write_bytes(subject)
    status, receipt = compile_delta(subject)
    assert status == 0, ("actual Epsilon source encoder", status, receipt[:80])
    executable = vm.compile_program(work, receipt, "literal-emitter", compiler=producer,
                                    realize=realize)
    status, fragment = vm.execute(executable, b"omega_is_identifier_start", seconds)
    assert status == 0 and fragment.endswith(b"; signed decimal edge: -9223372036854775808")
    (work / "literal-emitted.delta").write_bytes(fragment)
    # Keep only shared semantic declarations in the emitted program's runtime.
    manifest = root / "bootstrap/interpreted/4_epsilon/epsilon_compiler.delta.sources"
    representations = [line.split()[4] for line in manifest.read_text().splitlines()[1:]
                       if line.split()[4].startswith("representations/")]
    target = b"\n".join((manifest.parent / path).read_bytes() for path in representations)
    target += b"\n" + (root / "bootstrap/independent/epsilon/runtime/values.delta").read_bytes()
    target += b"\n" + fragment + b"\n" + (driver / "emitted_driver.delta").read_bytes()
    (work / "literal-target.delta").write_bytes(target)
    status, target_receipt = compile_delta(target)
    assert status == 0, ("emitted Delta program", status, target_receipt[:80])
    target_executable = vm.compile_program(work, target_receipt, "literal-target",
                                         compiler=producer, realize=realize)
    assert vm.execute(target_executable) == (0, b"*"), "emitted native function execution"
    result = {"positive_literals": len(positives) + 1,
              "malformed_literals": len(malformed), "role_type_refusals": 3,
              "emitter_source_bytes": len(subject), "fragment_bytes": len(fragment),
              "target_source_bytes": len(target)}
    (work / "literal-results.json").write_text(json.dumps(result, indent=2) + "\n")
    print("byte literals and actual Epsilon source encoder controls passed", flush=True)
    return result
