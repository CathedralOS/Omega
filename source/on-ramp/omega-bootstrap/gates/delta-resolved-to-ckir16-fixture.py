#!/usr/bin/env python3
"""Focused OMGRSW8/OMGLOWH source-to-CKIR16 u64 Less gate."""

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
FIXTURE = HERE / "fixtures/ckir16-u64-less/general.omg"
PACKAGE = "88" * 32
LOWER_HEADER = struct.Struct("<8sHHHH4I")
WITNESS_HEADER = struct.Struct("<8s4H17I")
WITNESS_WIDTHS = (36, 48, 28, 28, 24, 24, 24, 24, 28, 24, 40, 24, 40, 24)
WITNESS_NAMES = (
    "units", "imports", "bindings", "declarations", "types", "records", "fields",
    "sums", "cases", "payloads", "machines", "machine_parameters", "blocks",
    "block_parameters",
)

sys.path.insert(0, str(COMPILER))
sys.path.insert(0, str(HERE))
import checked_ir_v16_reference as ir16  # noqa: E402
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def owner_of(source: str) -> str:
    owners = re.findall(r"\bmachine\s+([A-Za-z_][A-Za-z0-9_]*)::run\b", source)
    require(len(owners) == 1, f"one selected ::run owner: {owners!r}")
    return owners[0]


def encode_source(source: str) -> bytes:
    packed = bundle.encode([bundle.Entry("main.omg", source.encode("ascii"))])
    manifest = {
        "target": "linux_x86_64",
        "packages": [{"key": PACKAGE, "sources": [{"label": "main.omg", "module": ""}]}],
        "aliases": [],
        "root": {"package": PACKAGE, "source": "main.omg",
                 "owner": owner_of(source), "machine": "run"},
    }
    return compilation.encode_manifest(manifest, packed)


def pack_lowering(comp: bytes, witness: bytes, *, version: int = 17,
                  selector: int = 8, magic: bytes = b"OMGLOWH\0") -> bytes:
    total = LOWER_HEADER.size + len(comp) + len(witness)
    require(len(comp) <= 267_280 and len(witness) <= 524_288 and total <= 791_600,
            "OMGLOWH component capacity")
    return LOWER_HEADER.pack(magic, version, 0, 0, LOWER_HEADER.size, total,
                             len(comp), len(witness), selector) + comp + witness


def run_status(executable: Path, contents: bytes, expected: int, name: str) -> bytes:
    try:
        result = subprocess.run([str(executable)], input=contents, stdout=subprocess.PIPE,
                                timeout=20)
    except subprocess.TimeoutExpired as error:
        raise ValueError(f"{name} did not converge within 20 seconds") from error
    require(result.returncode == expected,
            f"{name} status {result.returncode}, expected {expected}")
    if expected:
        require(not result.stdout, f"{name} published rejected bytes")
    return result.stdout


def self_host_source(path: Path) -> bytes:
    raw = re.sub(rb"//[^\n]*", b"", path.read_bytes())
    raw = re.sub(rb"\s+", b" ", raw)
    return re.sub(rb"\s*([^A-Za-z0-9_\s])\s*", rb"\1", raw)


def compile_tools(temp: Path) -> dict[str, Path]:
    resolver = COMPILER / "omega-bootstrap-resolve.alp"
    lowerer = COMPILER / "omega-bootstrap-resolved-to-ckir4.alp"
    lowermachine = REPO / "source/delta/samples/lowermachine.alp"
    delta_manifest = REPO / "source/on-ramp/rust/delta/Cargo.toml"
    delta = REPO / "source/on-ramp/rust/delta/target/debug/delta"
    subprocess.run(["cargo", "build", "-q", "--manifest-path", str(delta_manifest)], check=True)
    env = dict(os.environ, DELTA_ARCH="aarch64")
    result: dict[str, Path] = {}
    for name, source in (("resolver-native", resolver), ("lowerer-native", lowerer),
                         ("lowermachine", lowermachine)):
        destination = temp / name
        subprocess.run([str(delta), str(source), str(destination)], env=env, check=True,
                       stdout=subprocess.DEVNULL)
        result[name] = destination
    for stem, source in (("resolver", resolver), ("lowerer", lowerer)):
        assembly = temp / f"{stem}-self.s"
        with assembly.open("wb") as output:
            lowered = subprocess.run([str(result["lowermachine"])],
                                     input=self_host_source(source), stdout=output)
        require(lowered.returncode == 0, f"lowermachine rejected {stem}")
        destination = temp / f"{stem}-self"
        subprocess.run(["clang", "-arch", "arm64", "-o", str(destination), str(assembly)],
                       check=True)
        subprocess.run(["codesign", "-f", "-s", "-", str(destination)], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        result[f"{stem}-self"] = destination
    return result


def decode_witness(raw: bytes) -> dict[str, list[bytes]]:
    require(len(raw) >= WITNESS_HEADER.size, "truncated OMGRSW8")
    fixed = WITNESS_HEADER.unpack_from(raw)
    require(fixed[:5] == (b"OMGRSW8\0", 8, 0, 0, 84), "exact OMGRSW8 header")
    words = fixed[5:]
    require(words[0] == len(raw) and words[-1] == 0, "witness length/reserved")
    counts = (words[1], words[2], words[3], words[4], words[5], words[6], words[7],
              words[12], words[13], words[14], words[8], words[9], words[10], words[11])
    at = 84
    rows: dict[str, list[bytes]] = {}
    for name, count, width in zip(WITNESS_NAMES, counts, WITNESS_WIDTHS):
        end = at + count * width
        require(end <= len(raw), f"{name} extent")
        rows[name] = [raw[at + i * width:at + (i + 1) * width] for i in range(count)]
        at = end
    require(at == len(raw), "witness exact EOF")
    return rows


def inspect_witness(raw: bytes) -> None:
    types = [struct.unpack("<IBBHIIII", row) for row in decode_witness(raw)["types"]]
    full = [row for row in types if row[1:] ==
            (10, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF)]
    bounded = [row for row in types if row[1:] ==
               (10, 0, 0, 0, 0, 0xFFFF_FFFF, 1)]
    require(len(full) == 1, f"one normalized full-u64 row: {types!r}")
    require(len(bounded) == 1, f"one normalized borrow-bound row: {types!r}")


def inspect_ckir(raw: bytes) -> None:
    module = ir16.decode(raw)
    require(ir16.interpret(module) == 70, "CKIR16 interpreted result")
    require(len(ir16.selected_less(module)) == 1, "one selected kind-8 Less")
    types = module.tables["types"]
    require(sum(row[1:] == (8, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF)
                for row in types) == 1, "explicit full-u64 CKIR kind-8 row")
    require(sum(row[1:] == (8, 0, 0, 0, 0, 0xFFFF_FFFF, 1)
                for row in types) == 1, "explicit constrained CKIR kind-8 row")
    constants = [(row[10], row[11]) for row in module.tables["operations"]
                 if row[3] == 1 and types[row[7]][1] == 8]
    require((0xFFFF_FFFF, 1) in constants and (0, 2) in constants,
            f"two-word u64 constants: {constants!r}")
    require(any(row[3] == 6 for row in module.tables["operations"]), "u64 Store")
    calls = [row for row in module.tables["operations"] if row[3] == 10]
    require(any(types[row[7]][1] == 8 for row in calls), "kind-8 u64 Call result")


def positive_pipeline(tools: dict[str, Path], source: str) -> tuple[bytes, bytes, bytes]:
    comp = encode_source(source)
    witnesses: dict[str, bytes] = {}
    outputs: dict[str, bytes] = {}
    for mode in ("native", "self"):
        witnesses[mode] = run_status(tools[f"resolver-{mode}"], comp, 0,
                                     f"positive {mode} resolver")
        inspect_witness(witnesses[mode])
        outputs[mode] = run_status(
            tools[f"lowerer-{mode}"], pack_lowering(comp, witnesses[mode]), 0,
            f"positive {mode} lowerer",
        )
        inspect_ckir(outputs[mode])
    require(witnesses["native"] == witnesses["self"], "native/self witness parity")
    require(outputs["native"] == outputs["self"], "native/self CKIR parity")
    return comp, witnesses["native"], outputs["native"]


def resolver_reject(tools: dict[str, Path], name: str, source: str, status: int) -> None:
    comp = encode_source(source)
    for mode in ("native", "self"):
        run_status(tools[f"resolver-{mode}"], comp, status, f"{name} {mode} resolver")


def lowerer_reject(tools: dict[str, Path], name: str, source: str, status: int) -> None:
    comp = encode_source(source)
    for mode in ("native", "self"):
        witness = run_status(tools[f"resolver-{mode}"], comp, 0,
                             f"{name} {mode} resolver")
        run_status(tools[f"lowerer-{mode}"], pack_lowering(comp, witness), status,
                   f"{name} {mode} lowerer")


def run_gate() -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("resolved-to-CKIR16: skipped (requires Darwin arm64)")
        return
    source = FIXTURE.read_text(encoding="ascii")
    with tempfile.TemporaryDirectory(prefix="delta-resolved-to-ckir16-") as raw:
        tools = compile_tools(Path(raw))
        comp, witness, output = positive_pipeline(tools, source)

        resolver_reject(tools, "u64-trapping", source.replace("stored: u64;", "stored: u64 in Trapping;", 1), 251)
        unrelated = source.replace("transition self.stored < 8589934592", "transition self.byte < 2", 1).replace("stored: u64;", "stored: u64; byte: u32;", 1)
        resolver_reject(tools, "u64-plus-unrelated-u32-less", unrelated, 251)
        resolver_reject(tools, "mixed-carrier", source.replace("8589934592 {", "self.byte {", 1).replace("stored: u64;", "stored: u64; byte: u32;", 1), 251)
        resolver_reject(tools, "both-literals", source.replace("self.stored < 8589934592", "8589934591 < 8589934592", 1), 251)
        resolver_reject(tools, "nested-rhs", source.replace("8589934592 {", "8589934592 + 1 {", 1), 251)
        resolver_reject(tools, "parenthesized-suffix", source.replace(
            "self.stored < 8589934592 {", "(self.stored < 8589934592) + 1 {", 1), 251)
        resolver_reject(tools, "equality-prefix", source.replace(
            "self.stored < 8589934592 {", "false == self.stored < 8589934592 {", 1), 251)
        resolver_reject(tools, "less-equal-prefix", source.replace(
            "self.stored < 8589934592 {", "self.stored <= 8589934592 {", 1), 251)
        resolver_reject(tools, "overflow-literal", source.replace("8589934592 {", "18446744073709551616 {", 1), 251)
        resolver_reject(tools, "wide-array-length", source.replace("stored: u64;", "stored: u64; bytes: [u8; 4294967296];", 1), 252)

        wide_legacy = source.replace("self.stored = 8589934591;", "self.byte = 4294967296;\n    self.stored = 8589934591;", 1).replace("stored: u64;", "stored: u64; byte: u8;", 1)
        lowerer_reject(tools, "wide-legacy-literal", wide_legacy, 251)
        false_custody = source.replace("false -> failed()", "false -> bounded(self.stored)", 1)
        lowerer_reject(tools, "false-edge-no-fact", false_custody, 251)

        run_status(tools["lowerer-native"], pack_lowering(comp, witness, selector=7), 251,
                   "selector cross-pair")
        run_status(tools["lowerer-native"], pack_lowering(comp, witness, version=16,
                   magic=b"OMGLOWG\0"), 251, "outer identity cross-pair")
        relabeled = bytearray(output)
        struct.pack_into("<H", relabeled, 8, 15)
        try:
            ir16.decode(bytes(relabeled))
        except ir16.Ckir16Error:
            pass
        else:
            raise ValueError("CKIR16 relabeled major accepted")

        print("resolved-to-CKIR16: OMGRSW8/OMGLOWH native/self exact; kind10->kind8, "
              "two-word constants, direct pure u64 Less, borrow-bound true-edge custody, "
              "storage/call/edge transport, policy/mixed/literal/boundary/version negatives passed")


if __name__ == "__main__":
    try:
        run_gate()
    except (OSError, ValueError, struct.error, subprocess.CalledProcessError,
            bundle.BundleError, compilation.CompilationError,
            ir16.Ckir16Error) as error:
        raise SystemExit(f"resolved-to-CKIR16 fixture: {error}")
