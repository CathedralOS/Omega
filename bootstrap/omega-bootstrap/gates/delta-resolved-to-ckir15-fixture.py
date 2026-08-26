#!/usr/bin/env python3
"""Focused OMGRSW4/7 + OMGLOWG source-to-CKIR15 producer gate."""

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
FIXTURES = HERE / "fixtures/ckir15-recurrent-view"
LOWER_HEADER = struct.Struct("<8sHHHH4I")
PACKAGE = "88" * 32

sys.path.insert(0, str(COMPILER))
sys.path.insert(0, str(HERE))
import checked_ir_v15_reference as ir15  # noqa: E402
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


def pack_lowering(comp: bytes, witness: bytes, *, version: int = 16,
                  selector: int = 4, magic: bytes = b"OMGLOWG\0") -> bytes:
    require(len(comp) <= 267_280 and len(witness) <= 524_288,
            "OMGLOWG component capacity")
    total = LOWER_HEADER.size + len(comp) + len(witness)
    require(total <= 791_600, "OMGLOWG frame capacity")
    return LOWER_HEADER.pack(
        magic, version, 0, 0, LOWER_HEADER.size, total,
        len(comp), len(witness), selector,
    ) + comp + witness


def self_host_source(path: Path) -> bytes:
    raw = re.sub(rb"//[^\n]*", b"", path.read_bytes())
    raw = re.sub(rb"\s+", b" ", raw)
    return re.sub(rb"\s*([^A-Za-z0-9_\s])\s*", rb"\1", raw)


def run_status(executable: Path, contents: bytes, expected: int,
               name: str) -> bytes:
    result = subprocess.run([str(executable)], input=contents,
                            stdout=subprocess.PIPE)
    require(result.returncode == expected,
            f"{name} status {result.returncode}, expected {expected}")
    if expected:
        require(not result.stdout, f"{name} published rejection bytes")
    return result.stdout


def compile_tools(temp: Path) -> dict[str, Path]:
    resolver_source = COMPILER / "omega-bootstrap-resolve.alp"
    lowerer_source = COMPILER / "omega-bootstrap-resolved-to-ckir4.alp"
    lowermachine_source = REPO / "bootstrap/delta/samples/lowermachine.alp"
    delta_manifest = REPO / "bootstrap/delta/rust/Cargo.toml"
    delta = REPO / "bootstrap/delta/rust/target/debug/delta"
    for path in (resolver_source, lowerer_source, lowermachine_source):
        require(path.is_file(), f"missing {path}")

    subprocess.run(["cargo", "build", "-q", "--manifest-path", str(delta_manifest)],
                   check=True)
    env = dict(os.environ, DELTA_ARCH="aarch64")
    tools: dict[str, Path] = {}
    for name, source in (("resolver-native", resolver_source),
                         ("lowerer-native", lowerer_source),
                         ("lowermachine", lowermachine_source)):
        destination = temp / name
        subprocess.run([str(delta), str(source), str(destination)], env=env,
                       check=True, stdout=subprocess.DEVNULL)
        tools[name] = destination

    for stem, source in (("resolver", resolver_source), ("lowerer", lowerer_source)):
        assembly = temp / f"{stem}-self.s"
        with assembly.open("wb") as output:
            result = subprocess.run([str(tools["lowermachine"])],
                                    input=self_host_source(source), stdout=output)
        require(result.returncode == 0,
                f"lowermachine rejected {stem}: {result.returncode}")
        destination = temp / f"{stem}-self"
        subprocess.run(["clang", "-arch", "arm64", "-o", str(destination),
                        str(assembly)], check=True)
        subprocess.run(["codesign", "-f", "-s", "-", str(destination)],
                       check=True, stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL)
        tools[f"{stem}-self"] = destination
    return tools


def pipeline(tools: dict[str, Path], source: str, name: str,
             *, selector: int = 4, witness_magic: bytes = b"OMGRSW4\0") -> tuple[bytes, bytes, bytes]:
    comp = encode_source(source)
    witnesses: dict[str, bytes] = {}
    outputs: dict[str, bytes] = {}
    for mode in ("native", "self"):
        witness = run_status(tools[f"resolver-{mode}"], comp, 0,
                             f"{name} {mode} resolver")
        require(witness[:8] == witness_magic,
                f"{name} {mode} witness identity {witness[:8]!r}")
        output = run_status(
            tools[f"lowerer-{mode}"],
            pack_lowering(comp, witness, selector=selector), 0,
            f"{name} {mode} lowerer",
        )
        witnesses[mode] = witness
        outputs[mode] = output
    require(witnesses["native"] == witnesses["self"],
            f"{name} native/self witness divergence")
    require(outputs["native"] == outputs["self"],
            f"{name} native/self CKIR divergence")
    return comp, witnesses["native"], outputs["native"]


def inspect(output: bytes, name: str, *, static_views: int = 1) -> None:
    module = ir15.decode(output)
    require(ir15.interpret(module) == 70, f"{name} result")
    counts = ir15.selected_counts(module)
    require(counts == {22: static_views, 23: 2, 24: 2, 25: 2},
            f"{name} selected counts {counts}")
    synthetic = [row for row in module.tables["blocks"] if row[3] == 1]
    require(len(synthetic) == 2, f"{name} synthetic count")
    require(all(row[6] == 4 for row in synthetic),
            f"{name} synthetic (view,prefix,middle,suffix) arity")


def arithmetic_source(source: str) -> str:
    return source.replace(
        "data ByteProducer { result: u8; }",
        "data ByteProducer { result: u8; value: u32 in Trapping; }", 1
    ).replace(
        "state finish(&mut self, prefix: u8, middle: u8, suffix: u8) { self.result }",
        """state finish(&mut self, prefix: u8, middle: u8, suffix: u8) {
        self.value = self.result as u32 in Trapping;
        self.value = (self.value - 64) * 2 + (self.value - 70);
        transition self.value == 12 { true -> passed() false -> failed() }
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }""",
        1,
    )


def forced_rejection(tools: dict[str, Path], source: str, name: str) -> None:
    comp = encode_source(source)
    witness = run_status(tools["resolver-native"], comp, 0, f"{name} resolver")
    run_status(tools["lowerer-native"], pack_lowering(comp, witness), 251, name)


def run_gate() -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("resolved-to-CKIR15: skipped (requires Darwin arm64)")
        return

    sources = {path.stem: source_text(path) for path in sorted(FIXTURES.glob("*.omg"))}
    require(set(sources) == {"empty", "one-byte", "runtime-only", "two-byte"},
            "fixture set")
    with tempfile.TemporaryDirectory(prefix="delta-resolved-to-ckir15-") as raw:
        temp = Path(raw)
        tools = compile_tools(temp)
        positives: dict[str, tuple[bytes, bytes, bytes]] = {}
        for name in ("two-byte", "one-byte", "empty"):
            positives[name] = pipeline(tools, sources[name], name)
            inspect(positives[name][2], name)

        runtime_only = pipeline(tools, sources["runtime-only"], "runtime-only")
        inspect(runtime_only[2], "runtime-only", static_views=0)

        arithmetic = pipeline(
            tools, arithmetic_source(sources["two-byte"]), "selector-7-composition",
            selector=7, witness_magic=b"OMGRSW7\0",
        )
        inspect(arithmetic[2], "selector-7-composition")
        arithmetic_counts = ir15.selected_arithmetic_counts(ir15.decode(arithmetic[2]))
        require(all(arithmetic_counts[opcode] > 0 for opcode in (8, 26, 27)),
                f"selector-7 optional arithmetic composition {arithmetic_counts}")

        comp, witness, _ = positives["two-byte"]
        run_status(tools["lowerer-native"],
                   pack_lowering(comp, witness, version=15, magic=b"OMGLOWF\0"),
                   251, "old outer identity")
        run_status(tools["lowerer-native"], pack_lowering(comp, witness, selector=7),
                   251, "selector/witness cross-pair")
        run_status(tools["lowerer-native"],
                   pack_lowering(arithmetic[0], arithmetic[1], selector=4),
                   251, "arithmetic witness/selector cross-pair")

        base = sources["two-byte"]
        mutations = {
            "computed-pass": base.replace("emit(prefix, bytes[0]", "emit(prefix + 0, bytes[0]", 1),
            "duplicate-pass": base.replace(
                "emit(prefix, bytes[0], middle, bytes[1..], suffix)",
                "emit(prefix, bytes[0], prefix, bytes[1..], suffix)", 1),
            "false-reorder": base.replace(
                "false -> finish(prefix, middle, suffix)",
                "false -> finish(suffix, middle, prefix)", 1),
            "tail-before-head": base.replace(
                "emit(prefix, bytes[0], middle, bytes[1..], suffix)",
                "emit(prefix, bytes[1..], middle, bytes[0], suffix)", 1),
            "wrong-head": base.replace("bytes[0], middle", "bytes[1], middle", 1),
            "wrong-tail": base.replace("bytes[1..], suffix", "bytes[0..], suffix", 1),
            "one-occurrence": base.replace(
                """transition bytes.len > 0 {
            true -> emit(prefix, bytes[0], middle, bytes[1..], suffix)
            false -> finish(prefix, middle, suffix)
        }""",
                "transition { _ -> finish(prefix, middle, suffix) }", 1),
        }
        for name, source in mutations.items():
            forced_rejection(tools, source, name)

        oversized = base.replace('"GF"', '"' + "x" * 33 + '"', 1)
        run_status(tools["resolver-native"], encode_source(oversized), 252,
                   "33-byte literal")
        oversized_frame = LOWER_HEADER.pack(
            b"OMGLOWG\0", 16, 0, 0, LOWER_HEADER.size,
            LOWER_HEADER.size + 267_281, 267_281, 0, 4,
        )
        run_status(tools["lowerer-native"], oversized_frame, 252,
                   "OMGCOMP component ceiling")

        print("resolved-to-CKIR15: OMGLOWG/OMGRSW4+7 native/self exact; recurrent, "
              "one-byte, empty, and no-StaticByteView runtime-parameter carriers; "
              "ordered before/between/after pass binders and two synthetic head/tail "
              "edges; 251/252 controls passed")


if __name__ == "__main__":
    run_gate()
