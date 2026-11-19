#!/usr/bin/env python3
"""Construct a direct native Gamma compiler and its actual compiler customer.

The default route constructs through the audited Alpha/admitted Beta seed.
--accelerated explicitly adds the Python Gamma producer during development.
Python only joins source bytes, frames requests, and records process results in
normal construction. Generated successor executables need no Alpha seed.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "bootstrap/compiled_vm"))
import source_test as vm_source
import test as vm


def source(native):
    paths = [ROOT / "bootstrap/compiled_vm" / name
             for name in ("compiler.gamma", "frontend.gamma")]
    paths.append(ROOT / ("bootstrap/independent/native.gamma" if native
                         else "bootstrap/compiled_vm/alpha_backend.gamma"))
    return b"\n".join(path.read_bytes() for path in paths)


def identity(data):
    return {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def realize(artifact, executable):
    data = artifact.read_bytes()
    if data[:7] != b"\x7fELF\x02\x01\x01":
        raise AssertionError("native compiler failed to produce ELF64 little endian")
    shutil.copyfile(artifact, executable)
    executable.chmod(0o755)


def boundaries(work, native_source, compiler, seconds):
    """Private-profile mutations exercise limits; they are not construction edges."""
    import resource
    tiny_source = b"(def main () Int 42)"
    tiny = vm.compile_program(work, tiny_source, "boundary", compiler=compiler,
                              realize=realize)
    # This realization has a 128-byte ELF header and an 88-byte startup.
    code_size = tiny.stat().st_size - 216
    marker = b"(def i_limit () Int 134217513)"
    assert native_source.count(marker) == 1
    for limit, succeeds in ((code_size, False), (code_size + 1, True)):
        profile_source = native_source.replace(marker,
            f"(def i_limit () Int {limit})".encode("ascii"))
        profile = vm.compile_program(work, profile_source, f"capacity-{limit}",
                                     compiler=compiler, realize=realize)
        status, artifact = vm.execute(profile, tiny_source, seconds)
        if succeeds:
            assert status == 0 and artifact == tiny.read_bytes(), ("capacity adjacent", status)
        else:
            assert status == 254 and artifact == b"", ("capacity incomplete", status)

    def mapping_limit():
        resource.setrlimit(resource.RLIMIT_AS, (1 << 30, 1 << 30))

    for label, before in (("mmap", mapping_limit),
                          ("read", lambda: os.close(0)),
                          ("write", lambda: os.close(1))):
        result = subprocess.run([str(tiny)], input=b"", capture_output=True,
                                timeout=seconds, preexec_fn=before)
        assert (result.returncode, result.stdout, result.stderr) == (3, b"", b""), label
    print("native boundaries: exact/adjacent image capacity and mmap/read/write failures", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--accelerated", action="store_true")
    parser.add_argument("--seconds", type=int, default=3600)
    args = parser.parse_args()
    if args.seconds <= 0:
        parser.error("--seconds must be positive")
    if (platform.system(), platform.machine()) != ("Linux", "x86_64"):
        parser.error("native construction currently requires Linux x86-64")
    import resource
    mode = "accelerated" if args.accelerated else "reference"
    work = ROOT / "build" / ("independent-native-" + mode)
    work.mkdir(parents=True, exist_ok=True)
    vm_source.reconstruct(work)
    native_source = source(True)
    alpha_source = source(False)
    (work / "compiler.gamma").write_bytes(native_source)
    (work / "alpha-compiler.gamma").write_bytes(alpha_source)
    request = struct.pack("<I", len(native_source)) + native_source + native_source
    (work / "construction.request").write_bytes(request)
    started = time.monotonic()
    if args.accelerated:
        development = vm.compile_program(work, native_source, "development")
        status, native = vm.execute(development, native_source, args.seconds)
    else:
        status, native = vm.execute(work / "reference", request, args.seconds)
    assert status == 0, ("native compiler construction", status, len(native))
    (work / "compiler0.elf").write_bytes(native)
    realize(work / "compiler0.elf", work / "compiler0")
    elapsed = time.monotonic() - started
    print(f"{mode} compiler0: {identity(native)}; {elapsed:.3f}s", flush=True)
    for generation in (1, 2):
        status, rebuilt = vm.execute(work / f"compiler{generation-1}",
                                     native_source, args.seconds)
        assert status == 0 and rebuilt == native, ("native rebuild", generation, status)
        (work / f"compiler{generation}.elf").write_bytes(rebuilt)
        realize(work / f"compiler{generation}.elf", work / f"compiler{generation}")
    print("native compiler0 = compiler1 = compiler2: byte-exact", flush=True)

    class NativeCompiler:
        def __init__(self, subject):
            self.subject = subject

        def compile(self):
            status, result = vm.execute(work / "compiler2", self.subject, args.seconds)
            if status:
                assert result == b"" and status == 249, ("source refusal", status, len(result))
                raise vm.Invalid("Gamma source compiler rejected malformed source")
            return result

    vm.controls(work, work / "reference", compiler=NativeCompiler, realize=realize)
    boundaries(work, native_source, NativeCompiler, args.seconds)
    # The existing Alpha compiler is a real source-transforming successor. It
    # is compiled as written, not replaced by a source hash or precomputed tape.
    customer = vm.compile_program(work, alpha_source, "alpha-compiler",
                                 compiler=NativeCompiler, realize=realize)
    status, alpha_tape = vm.execute(customer, alpha_source, args.seconds)
    assert status == 0, ("actual Alpha compiler customer", status)
    (work / "alpha-compiler.tape").write_bytes(alpha_tape)
    vm.stamp(work / "alpha-compiler.tape", work / "alpha-compiler-vm")
    status, rebuilt_tape = vm.execute(work / "alpha-compiler-vm", alpha_source, args.seconds)
    assert status == 0 and rebuilt_tape == alpha_tape, ("cross-target compiler rebuild", status)

    class AlphaCompiler:
        def __init__(self, subject):
            self.subject = subject

        def compile(self):
            status, result = vm.execute(customer, self.subject, args.seconds)
            if status:
                assert result == b"" and status == 249, ("Alpha customer refusal", status)
                raise vm.Invalid("Gamma source compiler rejected malformed source")
            return result

    vm.controls(work, work / "reference", compiler=AlphaCompiler)
    report = {"mode": mode, "compiler_source": identity(native_source),
              "compiler_elf": identity(native), "construction_seconds": elapsed,
              "alpha_customer_source": identity(alpha_source),
              "alpha_customer_elf": identity(customer.read_bytes()),
              "alpha_customer_tape": identity(alpha_tape),
              "native_rebuilds": 2, "semantic_controls_each_target": 32,
              "refusal_controls_each_target": 15, "native_boundary_controls": 5,
              "children_maxrss_kib": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss}
    (work / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, sort_keys=True), flush=True)
    print("standalone native compiler and actual compiler customer passed", flush=True)


if __name__ == "__main__":
    main()
