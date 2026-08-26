#!/usr/bin/env python3
"""Focused OMGRSW4/OMGLOWD source-to-CKIR12 producer fixture and gate."""

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
FIXTURES = HERE / "fixtures/ckir12-static-byte-view"
ONE_BYTE_SOURCE = FIXTURES / "one-byte.omg"
EMPTY_SOURCE = FIXTURES / "empty.omg"
LOWER_HEADER = struct.Struct("<8sHHHH4I")
PACKAGE = "66" * 32

sys.path.insert(0, str(COMPILER))
sys.path.insert(0, str(HERE))
import checked_ir_v12_reference as ir12  # noqa: E402
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def source_text(path: Path) -> str:
    return path.read_text(encoding="ascii")


def encode_source(source: str) -> bytes:
    packed = bundle.encode([bundle.Entry("main.omg", source.encode("ascii"))])
    manifest = {
        "target": "linux_x86_64",
        "packages": [{
            "key": PACKAGE,
            "sources": [{"label": "main.omg", "module": ""}],
        }],
        "aliases": [],
        "root": {
            "package": PACKAGE, "source": "main.omg",
            "owner": "ByteProducer", "machine": "run",
        },
    }
    return compilation.encode_manifest(manifest, packed)


def pack_lowering(comp: bytes, witness: bytes, *, major: int = 13,
                  selector: int = 4, magic: bytes = b"OMGLOWD\0") -> bytes:
    require(len(comp) <= 267_280 and len(witness) <= 524_288,
            "OMGLOWD component capacity")
    total = LOWER_HEADER.size + len(comp) + len(witness)
    require(total <= 791_600, "OMGLOWD frame capacity")
    return LOWER_HEADER.pack(
        magic, major, 0, 0, LOWER_HEADER.size, total,
        len(comp), len(witness), selector,
    ) + comp + witness


def produce(resolver: Path, lowerer: Path, source: str) -> tuple[bytes, bytes, bytes]:
    comp = encode_source(source)
    resolved = subprocess.run([str(resolver)], input=comp, stdout=subprocess.PIPE)
    require(resolved.returncode == 0, f"resolver status {resolved.returncode}")
    require(resolved.stdout[:12] == b"OMGRSW4\0\x04\0\0\0",
            "resolver did not publish exact OMGRSW4")
    frame = pack_lowering(comp, resolved.stdout)
    lowered = subprocess.run([str(lowerer)], input=frame, stdout=subprocess.PIPE)
    require(lowered.returncode == 0, f"lowerer status {lowered.returncode}")
    return frame, resolved.stdout, lowered.stdout


def self_host_source(path: Path) -> bytes:
    raw = re.sub(rb"//[^\n]*", b"", path.read_bytes())
    raw = re.sub(rb"\s+", b" ", raw)
    return re.sub(rb"\s*([^A-Za-z0-9_\s])\s*", rb"\1", raw)


def inspect(contents: bytes, *, child_count: int) -> None:
    module = ir12.decode(contents)
    require(ir12.interpret(module) == 70, "CKIR12 result is not 70")
    opcodes = [row[3] for row in module.tables["operations"]]
    for opcode in range(22, 26):
        require(opcodes.count(opcode) == 1, f"opcode {opcode} count")
    synthetic = [row for row in module.tables["blocks"] if row[3] == 1]
    require(len(synthetic) == 1, "synthetic block count")
    require(synthetic[0][5] == 3 and synthetic[0][6] == 1,
            "synthetic block exact parameter span")
    require(len(module.tables["constant_children"]) == child_count,
            "literal child count")


def run_status(executable: Path, contents: bytes, expected: int, name: str) -> bytes:
    result = subprocess.run([str(executable)], input=contents, stdout=subprocess.PIPE)
    require(result.returncode == expected,
            f"{name} status {result.returncode}, expected {expected}")
    if expected:
        require(not result.stdout, f"{name} published rejection bytes")
    return result.stdout


def run_gate() -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("resolved-to-CKIR12: skipped (requires Darwin arm64)")
        return

    resolver_source = COMPILER / "omega-bootstrap-resolve.alp"
    lowerer_source = COMPILER / "omega-bootstrap-resolved-to-ckir4.alp"
    lowermachine_source = REPO / "bootstrap/delta/samples/lowermachine.alp"
    delta_manifest = REPO / "bootstrap/delta/rust/Cargo.toml"
    delta = REPO / "bootstrap/delta/rust/target/debug/delta"
    for path in (resolver_source, lowerer_source, lowermachine_source,
                 ONE_BYTE_SOURCE, EMPTY_SOURCE):
        require(path.is_file(), f"missing {path}")

    subprocess.run(["cargo", "build", "-q", "--manifest-path", str(delta_manifest)],
                   check=True)
    with tempfile.TemporaryDirectory(prefix="delta-resolved-to-ckir12-") as raw:
        temp = Path(raw)
        env = dict(os.environ, DELTA_ARCH="aarch64")
        for name, source in (("resolver", resolver_source), ("lowerer", lowerer_source),
                             ("lowermachine", lowermachine_source)):
            subprocess.run([str(delta), str(source), str(temp / name)], env=env,
                           check=True, stdout=subprocess.DEVNULL)
        for name, source in (("resolver-self", resolver_source),
                             ("lowerer-self", lowerer_source)):
            assembly = temp / f"{name}.s"
            with assembly.open("wb") as output:
                result = subprocess.run([str(temp / "lowermachine")],
                                        input=self_host_source(source), stdout=output)
            require(result.returncode == 0,
                    f"lowermachine rejected {name}: {result.returncode}")
            subprocess.run(["clang", "-arch", "arm64", "-o", str(temp / name),
                            str(assembly)], check=True)
            subprocess.run(["codesign", "-f", "-s", "-", str(temp / name)],
                           check=True, stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL)

        positives: dict[str, tuple[bytes, bytes, bytes]] = {}
        for name, path, children in (("one-byte", ONE_BYTE_SOURCE, 1),
                                     ("empty", EMPTY_SOURCE, 0)):
            source = source_text(path)
            native = produce(temp / "resolver", temp / "lowerer", source)
            self_built = produce(temp / "resolver-self", temp / "lowerer-self", source)
            require(native == self_built, f"{name} native/self byte divergence")
            inspect(native[2], child_count=children)
            positives[name] = native

        comp, witness, _ = positives["one-byte"]
        run_status(temp / "lowerer", pack_lowering(comp, witness, major=12,
                   magic=b"OMGLOWC\0"), 251, "old outer frame")
        run_status(temp / "lowerer", pack_lowering(comp, witness, selector=3),
                   251, "selector cross-pair")

        inherited_only = source_text(ONE_BYTE_SOURCE).replace(
            "transition view.len > 0 {\n            true -> present(view[0], view[1..])\n            false -> empty()\n        }",
            "transition { _ -> empty() }",
        )
        inherited_comp = encode_source(inherited_only)
        inherited_witness = run_status(temp / "resolver", inherited_comp, 0,
                                       "missing selected lowering resolver")
        run_status(temp / "lowerer", pack_lowering(inherited_comp, inherited_witness),
                   251, "missing selected lowering")

        for name, source in {
            "inclusive-nonempty": source_text(ONE_BYTE_SOURCE).replace(".len > 0", ".len >= 0"),
            "wrong-head": source_text(ONE_BYTE_SOURCE).replace("view[0],", "view[1],"),
            "wrong-tail": source_text(ONE_BYTE_SOURCE).replace("view[1..]", "view[0..]"),
        }.items():
            malformed_comp = encode_source(source)
            malformed_witness = run_status(temp / "resolver", malformed_comp, 0,
                                           f"{name} resolver")
            run_status(temp / "lowerer", pack_lowering(malformed_comp, malformed_witness),
                       251, name)

        oversized_source = source_text(ONE_BYTE_SOURCE).replace('"F"', '"' + "x" * 33 + '"')
        run_status(temp / "resolver", encode_source(oversized_source), 252,
                   "33-byte literal")

        oversized_frame = LOWER_HEADER.pack(
            b"OMGLOWD\0", 13, 0, 0, LOWER_HEADER.size,
            LOWER_HEADER.size + 267_281, 267_281, 0, 4,
        )
        run_status(temp / "lowerer", oversized_frame, 252,
                   "OMGCOMP component ceiling")

        print("resolved-to-CKIR12: OMGLOWD/OMGRSW4 native/self exact; producer-backed "
              "one-byte true edge and empty false bypass independently return 70; exact "
              "StaticByteView/SliceNonEmpty/SliceHead/SliceTailOne and synthetic block; "
              "focused 251/252 controls passed")


if __name__ == "__main__":
    run_gate()
