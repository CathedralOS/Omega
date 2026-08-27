#!/usr/bin/env python3
"""Focused OMGRSW7/OMGLOWF source-to-CKIR14 producer gate."""

from __future__ import annotations

import json
import os
import platform
import re
import struct
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
REPO = HERE.parents[2]
CASES = HERE / "fixtures/ckir14-arithmetic-cases"
LOWER_HEADER = struct.Struct("<8sHHHH4I")
PACKAGE = "77" * 32

sys.path.insert(0, str(COMPILER))
sys.path.insert(0, str(HERE))
import checked_ir_v5_reference as ir5  # noqa: E402
import checked_ir_v10_reference as ir10  # noqa: E402
import checked_ir_v12_reference as ir12  # noqa: E402
import checked_ir_v14_reference as ir14  # noqa: E402
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402
from omgrsw7_arithmetic_resolution_fixture import decode as decode_witness  # noqa: E402


@dataclass(frozen=True)
class SourceCase:
    name: str
    source: str
    counts: dict[int, int]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def owner_of(source: str) -> str:
    owners = re.findall(r"\bmachine\s+([A-Za-z_][A-Za-z0-9_]*)::run\b", source)
    require(len(owners) == 1, f"expected one selected ::run owner, got {owners!r}")
    return owners[0]


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
            "owner": owner_of(source), "machine": "run",
        },
    }
    return compilation.encode_manifest(manifest, packed)


def pack_lowering(comp: bytes, witness: bytes, *, version: int = 15,
                  selector: int = 7, magic: bytes = b"OMGLOWF\0") -> bytes:
    require(len(comp) <= 267_280 and len(witness) <= 524_288,
            "OMGLOW component capacity")
    total = LOWER_HEADER.size + len(comp) + len(witness)
    require(total <= 791_600, "OMGLOW frame capacity")
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


def exact_witness(witness: bytes, name: str = "witness") -> None:
    require(witness[:12] == b"OMGRSW7\0\x07\0\0\0",
            f"{name} exact OMGRSW7 identity, got {witness[:12]!r}")
    types = [struct.unpack("<IBBHIIII", row)
             for row in decode_witness(witness)["types"]]
    selected = [
        row for row in types
        if row[1:] == (2, 1, 0, 0, 0, 0, 0xFFFF_FFFF)
    ]
    require(len(selected) == 1, f"{name} unique exact full-u32 witness type")


def source_cases() -> dict[str, SourceCase]:
    manifest = json.loads((CASES / "manifest.json").read_text(encoding="utf-8"))
    result: dict[str, SourceCase] = {}
    for row in manifest["cases"]:
        arithmetic = row["arithmetic"]
        result[row["name"]] = SourceCase(
            row["name"],
            (CASES / row["file"]).read_text(encoding="ascii"),
            {8: arithmetic["add"], 26: arithmetic["subtract"],
             27: arithmetic["multiply"]},
        )
    return result


def selected_sequence(module: ir14.Module) -> list[int]:
    types = module.tables["types"]
    return [
        row[3] for row in module.tables["operations"]
        if row[3] in (8, 26, 27)
        and types[row[7]][1:] == (2, 1, 0, 0, 0, 0, 0xFFFF_FFFF)
    ]


def inspect_success(name: str, output: bytes, counts: dict[int, int],
                    *, sequence: list[int] | None = None) -> ir14.Module:
    module = ir14.decode(output)
    require(ir14.selected_counts(module) == counts,
            f"{name} selected operation correspondence")
    require(ir14.interpret(module) == 70, f"{name} result")
    if sequence is not None:
        require(selected_sequence(module) == sequence,
                f"{name} canonical postorder/precedence sequence")
    return module


def compile_tools(temp: Path) -> dict[str, Path]:
    resolver_source = COMPILER / "omega-bootstrap-resolve.alp"
    lowerer_source = COMPILER / "omega-bootstrap-resolved-to-ckir4.alp"
    lowermachine_source = REPO / "source/delta/samples/lowermachine.alp"
    delta_manifest = REPO / "source/on-ramp/rust/delta/Cargo.toml"
    delta = REPO / "source/on-ramp/rust/delta/target/debug/delta"
    for path in (resolver_source, lowerer_source, lowermachine_source):
        require(path.is_file(), f"missing {path}")

    subprocess.run(["cargo", "build", "-q", "--manifest-path", str(delta_manifest)],
                   check=True)
    env = dict(os.environ, DELTA_ARCH="aarch64")
    result: dict[str, Path] = {}
    for name, source in (("resolver-native", resolver_source),
                         ("lowerer-native", lowerer_source),
                         ("lowermachine", lowermachine_source)):
        destination = temp / name
        subprocess.run([str(delta), str(source), str(destination)], env=env,
                       check=True, stdout=subprocess.DEVNULL)
        result[name] = destination

    for stem, source in (("resolver", resolver_source), ("lowerer", lowerer_source)):
        assembly = temp / f"{stem}-self.s"
        with assembly.open("wb") as output:
            lowered = subprocess.run([str(result["lowermachine"])],
                                     input=self_host_source(source), stdout=output)
        require(lowered.returncode == 0,
                f"lowermachine rejected {stem}: {lowered.returncode}")
        destination = temp / f"{stem}-self"
        subprocess.run(["clang", "-arch", "arm64", "-o", str(destination),
                        str(assembly)], check=True)
        subprocess.run(["codesign", "-f", "-s", "-", str(destination)],
                       check=True, stdout=subprocess.DEVNULL,
                       stderr=subprocess.DEVNULL)
        result[f"{stem}-self"] = destination
    return result


def pipelines(tools: dict[str, Path], source: str, name: str) -> tuple[bytes, bytes, bytes]:
    comp = encode_source(source)
    witnesses: dict[str, bytes] = {}
    outputs: dict[str, bytes] = {}
    for mode in ("native", "self"):
        witness = run_status(tools[f"resolver-{mode}"], comp, 0,
                             f"{name} {mode} resolver")
        exact_witness(witness, f"{name} {mode}")
        output = run_status(tools[f"lowerer-{mode}"], pack_lowering(comp, witness), 0,
                            f"{name} {mode} lowerer")
        witnesses[mode] = witness
        outputs[mode] = output
    require(witnesses["native"] == witnesses["self"],
            f"{name} native/self witness divergence")
    require(outputs["native"] == outputs["self"],
            f"{name} native/self CKIR divergence")
    return comp, witnesses["native"], outputs["native"]


def forced_rejection(tools: dict[str, Path], source: str, expected: int,
                     name: str, *, require_legacy_witness: bool = False) -> tuple[bytes, bytes]:
    comp = encode_source(source)
    witnesses: dict[str, bytes] = {}
    for mode in ("native", "self"):
        witness = run_status(tools[f"resolver-{mode}"], comp, 0,
                             f"{name} {mode} resolver")
        if require_legacy_witness:
            require(witness[:8] != b"OMGRSW7\0",
                    f"{name} incorrectly selected OMGRSW7")
        run_status(tools[f"lowerer-{mode}"], pack_lowering(comp, witness), expected,
                   f"{name} {mode} lowerer")
        witnesses[mode] = witness
    require(witnesses["native"] == witnesses["self"],
            f"{name} native/self rejection-witness divergence")
    return comp, witnesses["native"]


FULL_LITERAL = """data LiteralBoundary { result: u32 in Trapping; }
machine LiteralBoundary::run(&mut self) -> u8 {
    self.result = 4294967295 + 0;
    transition self.result == 4294967295 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""

OVERFLOW_LITERAL = FULL_LITERAL.replace("4294967295 + 0", "4294967296 + 0", 1)


def contextual_literal_source(value: int) -> str:
    return f"""data ContextualLiteral {{ result: u32 in Trapping; }}
machine ContextualLiteral::run(&mut self) -> u8 {{
    self.result = {value} + 0;
    transition self.result == {value} {{ true -> passed() false -> failed() }}
    state passed(&mut self) {{ 70 }}
    state failed(&mut self) {{ 0 }}
}}
"""

WIDEN_ARITHMETIC = """data WidenArithmetic { byte: u8; result: u32 in Trapping; }
machine WidenArithmetic::run(&mut self) -> u8 {
    self.byte = 69;
    self.result = (self.byte as u32 in Trapping) + 1;
    transition self.result == 70 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""

WIDEN_ONLY = WIDEN_ARITHMETIC.replace(
    "(self.byte as u32 in Trapping) + 1", "self.byte as u32 in Trapping", 1
).replace("self.byte = 69", "self.byte = 70", 1)


def old_producer_regressions(tools: dict[str, Path], cases: dict[str, SourceCase],
                             new_comp: bytes, new_witness: bytes,
                             new_output: bytes) -> None:
    # Exact widening without selected arithmetic retains OMGRSW1 + OMGLOWB -> CKIR10.
    widen_comp = encode_source(WIDEN_ONLY)
    widen_witness = run_status(tools["resolver-native"], widen_comp, 0,
                               "CKIR10 resolver regression")
    require(widen_witness[:8] == b"OMGRSW1\0", "CKIR10 least witness regression")
    widen_output = run_status(
        tools["lowerer-native"],
        pack_lowering(widen_comp, widen_witness, version=11,
                      selector=1, magic=b"OMGLOWB\0"),
        0, "CKIR10 lowerer regression",
    )
    require(ir10.interpret(ir10.decode(widen_output)) == 70,
            "CKIR10 historical target result")

    # The independent static-view source retains OMGRSW4 + OMGLOWD -> CKIR12.
    view_only = cases["ckir12-view-plus-arithmetic"].source.replace(
        "        self.value = (self.value - 64) * 2 + (self.value - 70);\n", "", 1
    ).replace(
        "transition self.value == 12", "transition self.value == 70", 1
    )
    view_comp = encode_source(view_only)
    view_witness = run_status(tools["resolver-native"], view_comp, 0,
                              "CKIR12 resolver regression")
    require(view_witness[:8] == b"OMGRSW4\0", "CKIR12 least witness regression")
    view_output = run_status(
        tools["lowerer-native"],
        pack_lowering(view_comp, view_witness, version=13,
                      selector=4, magic=b"OMGLOWD\0"),
        0, "CKIR12 lowerer regression",
    )
    require(ir12.interpret(ir12.decode(view_output)) == 70,
            "CKIR12 historical target result")

    run_status(tools["lowerer-native"],
               pack_lowering(new_comp, new_witness, version=14,
                             magic=b"OMGLOWE\0"),
               251, "retired OMGLOWE identity")
    run_status(tools["lowerer-native"],
               pack_lowering(new_comp, new_witness, selector=4),
               251, "OMGLOWF selector cross-pair")
    retired = bytearray(new_witness)
    retired[:8] = b"OMGRSW5\0"
    struct.pack_into("<H", retired, 8, 5)
    run_status(tools["lowerer-native"],
               pack_lowering(new_comp, bytes(retired)),
               251, "retired OMGRSW5 witness")

    relabeled = bytearray(new_output)
    struct.pack_into("<H", relabeled, 8, 12)
    try:
        ir12.decode(bytes(relabeled))
    except ir5.Ckir5Error:
        pass
    else:
        raise ValueError("CKIR14 payload relabeled as CKIR12 was accepted")


def run_gate() -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("resolved-to-CKIR14: skipped (requires Darwin arm64)")
        return

    cases = source_cases()
    with tempfile.TemporaryDirectory(prefix="delta-resolved-to-ckir14-") as raw:
        temp = Path(raw)
        tools = compile_tools(temp)

        outputs: dict[str, tuple[bytes, bytes, bytes]] = {}
        for name in (
            "add-only-selected", "precedence-association-parentheses",
            "utf8-4-byte", "representative-contexts", "depth-8-boundary",
            "ckir12-view-plus-arithmetic",
        ):
            case = cases[name]
            outputs[name] = pipelines(tools, case.source, name)
            sequence = None
            if name == "add-only-selected":
                sequence = [8]
            elif name == "precedence-association-parentheses":
                sequence = [27, 26, 8, 26, 8, 26, 27, 8]
            module = inspect_success(name, outputs[name][2], case.counts,
                                     sequence=sequence)
            if name == "ckir12-view-plus-arithmetic":
                view_counts = ir12.selected_counts(module)
                require(view_counts == {22: 1, 23: 1, 24: 1, 25: 1},
                        f"complete optional CKIR12 view composition: {view_counts}")

        literal = pipelines(tools, FULL_LITERAL, "full-width-literal")
        literal_module = inspect_success("full-width-literal", literal[2],
                                         {8: 1, 26: 0, 27: 0}, sequence=[8])
        full_type = next(row[0] for row in literal_module.tables["types"]
                         if row[1:] == (2, 1, 0, 0, 0, 0, 0xFFFF_FFFF))
        require(any(row[3] == 1 and row[7] == full_type and row[10] == 0xFFFF_FFFF
                    for row in literal_module.tables["operations"]),
                "0xffffffff Const semantic word")

        for value in (0x8000_0000, 0xFFFF_FFFE):
            name = f"contextual-literal-{value:08x}"
            boundary = pipelines(tools, contextual_literal_source(value), name)
            boundary_module = inspect_success(
                name, boundary[2], {8: 1, 26: 0, 27: 0}, sequence=[8]
            )
            boundary_type = next(
                row[0] for row in boundary_module.tables["types"]
                if row[1:] == (2, 1, 0, 0, 0, 0, 0xFFFF_FFFF)
            )
            require(any(
                row[3] == 1 and row[7] == boundary_type and row[10] == value
                for row in boundary_module.tables["operations"]
            ), f"{name} exact opcode-1 immediate")

        widened = pipelines(tools, WIDEN_ARITHMETIC, "widened-u8-arithmetic")
        widened_module = inspect_success("widened-u8-arithmetic", widened[2],
                                         {8: 1, 26: 0, 27: 0}, sequence=[8])
        require(sum(row[3] == 21 for row in widened_module.tables["operations"]) == 1,
                "one inherited IntegerWiden primary")

        for name, message in (
            ("nested-underflow", "runtime subtract range"),
            ("multiply-overflow", "runtime multiply range"),
            ("add-overflow", "runtime add range"),
        ):
            case = cases[name]
            output = pipelines(tools, case.source, name)[2]
            module = ir14.decode(output)
            require(ir14.selected_counts(module) == case.counts,
                    f"{name} operation correspondence")
            try:
                ir14.interpret(module)
            except ir5.Ckir5Error as error:
                require(message in str(error), f"{name} wrong trap: {error}")
            else:
                raise ValueError(f"{name} published a machine result")

        forced_rejection(tools, cases["missing-arithmetic"].source, 251,
                         "missing-arithmetic", require_legacy_witness=True)
        forced_rejection(tools, cases["depth-9-exhausted"].source, 252,
                         "depth-9-exhausted")
        wrong_domain_comp = encode_source(cases["wrong-domain"].source)
        for mode in ("native", "self"):
            run_status(tools[f"resolver-{mode}"], wrong_domain_comp, 251,
                       f"wrong-domain {mode} resolver")
        forced_rejection(tools, cases["unsupported-divide"].source, 251,
                         "unsupported-divide", require_legacy_witness=True)

        for name, source in (
            ("bare-widen-cast", WIDEN_ARITHMETIC.replace(
                "as u32 in Trapping", "as u32", 1)),
            ("wrong-policy-widen-cast", WIDEN_ARITHMETIC.replace(
                "as u32 in Trapping", "as u32 in Saturating", 1)),
        ):
            forced_rejection(tools, source, 251, name)

        overflow_comp = encode_source(OVERFLOW_LITERAL)
        for mode in ("native", "self"):
            run_status(tools[f"resolver-{mode}"], overflow_comp, 251,
                       f"overflow-literal {mode} resolver")

        run_status(
            tools["lowerer-native"],
            pack_lowering(outputs["add-only-selected"][0],
                          outputs["precedence-association-parentheses"][1]),
            251, "exact source/witness cross-pair",
        )

        old_producer_regressions(
            tools, cases, outputs["add-only-selected"][0],
            outputs["add-only-selected"][1], outputs["add-only-selected"][2],
        )

        print(
            "resolved-to-CKIR14: OMGLOWF/OMGRSW7 native/self exact; Add/Subtract/"
            "Multiply recursion, precedence/association/postorder, exact 0x80000000/"
            "0xfffffffe/0xffffffff contextual literals, "
            "exact widening primary, contexts, depth 8/9, three node traps without "
            "machine result, optional CKIR12 view composition, missing/excluded forms, "
            "and CKIR10/12 plus retired/cross-version regressions passed"
        )


if __name__ == "__main__":
    try:
        run_gate()
    except (OSError, ValueError, struct.error, subprocess.CalledProcessError,
            bundle.BundleError, compilation.CompilationError,
            ir5.Ckir5Error) as error:
        raise SystemExit(f"resolved-to-CKIR14 fixture: {error}")
