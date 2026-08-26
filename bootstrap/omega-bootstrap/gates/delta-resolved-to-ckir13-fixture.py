#!/usr/bin/env python3
"""Focused OMGRSW5/OMGLOWE source-to-CKIR13 subtraction producer gate."""

from __future__ import annotations

import os
import platform
import re
import struct
import subprocess
import sys
import tempfile
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
REPO = HERE.parents[2]
FIXTURES = HERE / "fixtures/ckir13-full-u32-subtract"
SUCCESS = FIXTURES / "success.omg"
UNDERFLOW = FIXTURES / "underflow.omg"
GUARD = FIXTURES / "guard.omg"
CALL_ARGUMENT = FIXTURES / "call-argument.omg"
TRANSITION_ARGUMENT = FIXTURES / "transition-argument.omg"
LITERAL_SUCCESS = FIXTURES / "literal-success.omg"
LOWER_HEADER = struct.Struct("<8sHHHH4I")
PACKAGE = "6d" * 32
WIDTHS = (
    ("units", 36), ("imports", 48), ("bindings", 28),
    ("declarations", 28), ("types", 24), ("records", 24),
    ("fields", 24), ("sums", 24), ("cases", 28), ("payloads", 24),
    ("machines", 40), ("machine_parameters", 24),
    ("blocks", 40), ("block_parameters", 24),
)

sys.path.insert(0, str(COMPILER))
sys.path.insert(0, str(HERE))
import checked_ir_v13_reference as ir13  # noqa: E402
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def encode_source(source: str) -> bytes:
    packed = bundle.encode([bundle.Entry("main.omg", source.encode("ascii"))])
    manifest = {
        "target": "linux_x86_64",
        "packages": [{"key": PACKAGE,
                      "sources": [{"label": "main.omg", "module": ""}]}],
        "aliases": [],
        "root": {"package": PACKAGE, "source": "main.omg",
                 "owner": "SubProducer", "machine": "run"},
    }
    return compilation.encode_manifest(manifest, packed)


def pack_lowering(comp: bytes, witness: bytes, *, major: int = 14,
                  selector: int = 5, magic: bytes = b"OMGLOWE\0") -> bytes:
    total = LOWER_HEADER.size + len(comp) + len(witness)
    return LOWER_HEADER.pack(magic, major, 0, 0, LOWER_HEADER.size, total,
                             len(comp), len(witness), selector) + comp + witness


def run_status(executable: Path, contents: bytes, expected: int, name: str) -> bytes:
    result = subprocess.run([str(executable)], input=contents, stdout=subprocess.PIPE)
    require(result.returncode == expected,
            f"{name}: status {result.returncode}, expected {expected}")
    if expected:
        require(not result.stdout, f"{name}: rejection published bytes")
    return result.stdout


def inspect_witness(raw: bytes) -> None:
    require(len(raw) >= 84 and raw[:16] == b"OMGRSW5\0\x05\0\0\0\0\0T\0",
            "exact OMGRSW5 header")
    words = struct.unpack_from("<17I", raw, 16)
    require(words[0] == len(raw), "OMGRSW5 exact length")
    names = ("sources", "imports", "bindings", "declarations", "types", "records",
             "fields", "machines", "machine_parameters", "blocks", "block_parameters",
             "sums", "cases", "payloads", "selected", "reserved")
    counts = dict(zip(names, words[1:]))
    at = 84
    type_rows: list[tuple[int, ...]] = []
    table_counts = (counts["sources"], counts["imports"], counts["bindings"],
                    counts["declarations"], counts["types"], counts["records"],
                    counts["fields"], counts["sums"], counts["cases"],
                    counts["payloads"], counts["machines"],
                    counts["machine_parameters"], counts["blocks"],
                    counts["block_parameters"])
    for (name, width), count in zip(WIDTHS, table_counts):
        end = at + width * count
        require(end <= len(raw), f"OMGRSW5 {name} extent")
        if name == "types":
            type_rows = [struct.unpack_from("<IBBHIIII", raw, at + width * i)
                         for i in range(count)]
        at = end
    require(at == len(raw), "OMGRSW5 exact EOF")
    require(any(row[1:] == (2, 1, 0, 0, 0, 0, 0xFFFF_FFFF)
                for row in type_rows), "canonical full u32 in Trapping")


def mutate_scalar_high(raw: bytes, kind: int, high: int) -> bytes:
    words = struct.unpack_from("<17I", raw, 16)
    type_count = words[5]
    at = 84 + words[1] * 36 + words[2] * 48 + words[3] * 28 + words[4] * 28
    changed = bytearray(raw)
    for row in range(type_count):
        row_at = at + row * 24
        if changed[row_at + 4] == kind:
            struct.pack_into("<I", changed, row_at + 20, high)
            return bytes(changed)
    raise ValueError(f"missing scalar kind {kind}")


def self_host_source(path: Path) -> bytes:
    raw = re.sub(rb"//[^\n]*", b"", path.read_bytes())
    raw = re.sub(rb"\s+", b" ", raw)
    return re.sub(rb"\s*([^A-Za-z0-9_\s])\s*", rb"\1", raw)


def produce(resolver: Path, lowerer: Path, source: str, name: str = "producer") -> tuple[bytes, bytes]:
    comp = encode_source(source)
    witness = run_status(resolver, comp, 0, f"{name} resolver")
    inspect_witness(witness)
    ckir = run_status(lowerer, pack_lowering(comp, witness), 0, f"{name} lowerer")
    module = ir13.decode(ckir)
    require(ir13.selected_subtract_count(module) == 1, "one selected subtraction")
    return witness, ckir


def run_gate() -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("resolved-to-CKIR13: skipped (requires Darwin arm64)")
        return
    resolver_source = COMPILER / "omega-bootstrap-resolve.alp"
    lowerer_source = COMPILER / "omega-bootstrap-resolved-to-ckir4.alp"
    lowermachine_source = REPO / "bootstrap/rungs/delta/samples/lowermachine.alp"
    delta_manifest = REPO / "bootstrap/onramps/delta-rust/Cargo.toml"
    delta = REPO / "bootstrap/onramps/delta-rust/target/debug/delta"
    for path in (resolver_source, lowerer_source, lowermachine_source, SUCCESS,
                 UNDERFLOW, GUARD, CALL_ARGUMENT, TRANSITION_ARGUMENT, LITERAL_SUCCESS):
        require(path.is_file(), f"missing {path}")

    subprocess.run(["cargo", "build", "-q", "--manifest-path", str(delta_manifest)], check=True)
    with tempfile.TemporaryDirectory(prefix="delta-resolved-to-ckir13-") as raw:
        temp = Path(raw)
        env = dict(os.environ, DELTA_ARCH="aarch64")
        for name, source in (("resolver", resolver_source), ("lowerer", lowerer_source),
                             ("lowermachine", lowermachine_source)):
            subprocess.run([str(delta), str(source), str(temp / name)], env=env,
                           check=True, stdout=subprocess.DEVNULL)
        for name, source in (("resolver.self", resolver_source),
                             ("lowerer.self", lowerer_source)):
            assembly = temp / f"{name}.s"
            with assembly.open("wb") as output:
                result = subprocess.run([str(temp / "lowermachine")],
                                        input=self_host_source(source), stdout=output)
            require(result.returncode == 0, f"lowermachine rejected {name}")
            subprocess.run(["clang", "-arch", "arm64", "-o", str(temp / name),
                            str(assembly)], check=True)
            subprocess.run(["codesign", "-f", "-s", "-", str(temp / name)], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        success = SUCCESS.read_text(encoding="ascii")
        underflow = UNDERFLOW.read_text(encoding="ascii")
        call_argument = CALL_ARGUMENT.read_text(encoding="ascii")
        positives = (
            ("assignment-leaf", success, 70),
            ("assignment-literal", LITERAL_SUCCESS.read_text(encoding="ascii"), 70),
            ("guard", GUARD.read_text(encoding="ascii"), 70),
            ("call-argument", call_argument, 70),
            ("transition-argument", TRANSITION_ARGUMENT.read_text(encoding="ascii"), 70),
            ("underflow", underflow, None),
        )
        for name, source, expected in positives:
            native = produce(temp / "resolver", temp / "lowerer", source, name)
            self_built = produce(temp / "resolver.self", temp / "lowerer.self", source, name)
            require(native == self_built, f"{name}: native/self byte divergence")
            if expected is None:
                try:
                    ir13.interpret(ir13.decode(native[1]))
                except ir13.Ckir13Error:
                    pass
                else:
                    raise ValueError("underflow did not trap")
            else:
                require(ir13.interpret(ir13.decode(native[1])) == expected,
                        f"{name}: independent result")

        low_subtract = underflow.replace("self.cursor = 0;", "self.cursor = 7;")
        low_witness = run_status(temp / "resolver", encode_source(low_subtract), 0,
                                 "low-literal subtraction")
        require(low_witness[:12] == b"OMGRSW5\0\x05\0\0\0",
                "low-literal subtraction must select OMGRSW5")

        inherited = underflow.replace("self.cursor = self.cursor - 1;", "self.cursor = 1;")
        inherited_witness = run_status(temp / "resolver", encode_source(inherited), 0,
                                       "inherited source")
        inherited_self = run_status(temp / "resolver.self", encode_source(inherited), 0,
                                    "inherited source self")
        require(inherited_witness == inherited_self, "inherited native/self witness bytes")
        require(inherited_witness[:7] in (b"OMGRSW1", b"OMGRSW2", b"OMGRSW3", b"OMGRSW4"),
                "least inherited resolver version")

        run_status(temp / "resolver",
                   encode_source(success.replace("4294967295", "4294967296")), 251,
                   "decimal above u32")

        comp = encode_source(success)
        witness = run_status(temp / "resolver", comp, 0, "cross-pair resolver")
        for name, frame in {
            "old outer": pack_lowering(comp, witness, major=13, magic=b"OMGLOWD\0"),
            "wrong selector": pack_lowering(comp, witness, selector=4),
        }.items():
            run_status(temp / "lowerer", frame, 251, name)
        run_status(temp / "lowerer",
                   pack_lowering(comp, mutate_scalar_high(witness, 1, 0xFFFF_FFFF)),
                   251, "full-width u8 witness")
        for name, comp_size, witness_size in (
            ("OMGCOMP component ceiling", 267_281, 0),
            ("OMGRSW component ceiling", 0, 524_289),
        ):
            total = LOWER_HEADER.size + comp_size + witness_size
            oversized = LOWER_HEADER.pack(b"OMGLOWE\0", 14, 0, 0,
                                          LOWER_HEADER.size, total,
                                          comp_size, witness_size, 5)
            run_status(temp / "lowerer", oversized, 252, name)

        for name, source in {
            "literal-left": underflow.replace("self.cursor - 1", "1 - self.cursor"),
            "wrong-domain": underflow.replace("u32 in Trapping", "u32 in Exact"),
            "nested-left": success.replace("self.cursor - self.floor",
                                            "self.cursor + self.floor - self.floor"),
            "two-trapping-call-arguments": call_argument.replace(
                "prefix: u8, value: u32 in Trapping",
                "prefix: u32 in Trapping, value: u32 in Trapping",
            ).replace(
                "self.gate.keep(7, self.cursor - self.floor, 9)",
                "self.gate.keep(self.cursor - self.floor, self.cursor - self.floor, 9)",
            ),
        }.items():
            bad_comp = encode_source(source)
            resolved = subprocess.run([str(temp / "resolver")], input=bad_comp,
                                      stdout=subprocess.PIPE)
            require(resolved.returncode in (0, 251), f"{name}: resolver status")
            if resolved.returncode == 251:
                require(not resolved.stdout, f"{name}: resolver rejection publication")
            else:
                run_status(temp / "lowerer", pack_lowering(bad_comp, resolved.stdout),
                           251, name)

        print("resolved-to-CKIR13: OMGRSW5/OMGLOWE native/self exact; full-u32 "
              "leaf-leaf and leaf-literal subtraction in assignment, guard, call, and "
              "transition arguments; maximum custody, runtime underflow, least-version, "
              "cross-pair, shape/domain/one-trapping-call-argument controls passed")


if __name__ == "__main__":
    run_gate()
