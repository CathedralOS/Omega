#!/usr/bin/env python3
"""Focused OMGLOW6 source-to-CKIR5 producer gate."""

from __future__ import annotations

import os
import platform
import re
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
sys.path.insert(0, str(COMPILER))
sys.path.insert(0, str(HERE))

import checked_ir_v5_reference as ir5  # noqa: E402
import checked_ir_v6_reference as ir6  # noqa: E402
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


PACKAGE = "55" * 32
LOWER_HEADER = struct.Struct("<8sHHHH4I")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def encode_source(source: str, owner: str = "SumProducer", machine: str = "run") -> bytes:
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
            "owner": owner, "machine": machine,
        },
    }
    return compilation.encode_manifest(manifest, packed)


def pack_lowering(comp: bytes, witness: bytes, version: int = 6,
                  resolution: int = 0) -> bytes:
    require(len(comp) <= 267_280 and len(witness) <= 524_288, "OMGLOW component capacity")
    require(version in (4, 5, 6, 7), "test lowering version")
    require((version == 7 and resolution in (1, 2, 3))
            or (version != 7 and resolution == 0), "test resolution selector")
    total = LOWER_HEADER.size + len(comp) + len(witness)
    require(total <= 791_600, "OMGLOW frame capacity")
    return LOWER_HEADER.pack(
        f"OMGLOW{version}".encode("ascii") + b"\0", version, 0, 0,
        LOWER_HEADER.size, total, len(comp), len(witness), resolution,
    ) + comp + witness


def declaration_source(members: str) -> str:
    return f"""data Nested [copy] {{ left: u8; right: u8; }}
data Wide [copy] {{
{members}
}}
data SumProducer {{}}
machine SumProducer::run(&mut self) -> u8 {{ 70 }}
"""


def resource_sources() -> dict[str, tuple[str, int]]:
    cases_64 = "\n".join(f"    case C{index};" for index in range(64))
    cases_65 = "\n".join(f"    case C{index};" for index in range(65))
    return {
        "declarations-single-payload": (declaration_source("""
    case Empty;
    case One(value: u8);
"""), 0),
        "declarations-0-through-4": (declaration_source("""
    case Empty;
    case One(a: u8);
    case Two(a: u8, b: u8);
    case Three(a: bool, b: bool, c: bool);
    case Four(a: u8, b: u8, nested: Nested, d: u8);
"""), 0),
        "payload-five": (declaration_source(
            "    case Five(a: u8, b: u8, c: u8, d: u8, e: u8);"
        ), 252),
        "payload-five-malformed": (declaration_source(
            "    case Five(a: u8, b: u8, c: u8, d: u8, a: u8);"
        ), 251),
        "cases-64": (declaration_source(cases_64), 0),
        "cases-65": (declaration_source(cases_65), 252),
    }


def lowering_negatives(general: str) -> dict[str, str]:
    result = {
        "construct-missing": general.replace(
            "Packet::One { value: 1 };", "Packet::One {};", 1
        ),
        "construct-duplicate": general.replace(
            "Packet::One { value: 1 };", "Packet::One { value: 1, value: 2 };", 1
        ),
        "construct-unknown": general.replace(
            "Packet::One { value: 1 };", "Packet::One { nope: 1 };", 1
        ),
        "construct-mistyped": general.replace(
            "Packet::One { value: 1 };", "Packet::One { value: true };", 1
        ),
        "arm-nonexhaustive": general.replace(
            "        Packet::Empty -> consume_empty()\n", "", 1
        ),
        "arm-duplicate": general.replace(
            "        Packet::Empty -> consume_empty()\n",
            "        Packet::Empty -> consume_empty()\n        Packet::Empty -> consume_empty()\n",
            1,
        ),
        "arm-incomplete-payload": general.replace(
            "Packet::Four { a, b, nested, tail } -> consume_four(a, b, nested, tail)",
            "Packet::Four { a } -> consume_one(a)",
            1,
        ),
    }
    result["construct-effectful"] = general.replace(
        "machine SumProducer::run(&mut self) -> u8 {",
        "machine SumProducer::scalar(&self) -> u8 { 1 }\n\nmachine SumProducer::run(&mut self) -> u8 {",
        1,
    ).replace("Packet::One { value: 1 };", "Packet::One { value: self.scalar() };", 1)
    result["arm-wrong-owner"] = general.replace(
        "data SumProducer {",
        "data Other [copy] { case Empty; }\n\ndata SumProducer {",
        1,
    ).replace("Packet::Empty -> consume_empty()", "Other::Empty -> consume_empty()", 1)
    return result


def self_host_source(path: Path) -> bytes:
    raw = re.sub(rb"//[^\n]*", b"", path.read_bytes())
    raw = re.sub(rb"\s+", b" ", raw)
    # Lowermachine's bounded source arena is 262,144 bytes. Whitespace is
    # significant only between adjacent word tokens in this Delta source.
    return re.sub(rb"\s*([^A-Za-z0-9_\s])\s*", rb"\1", raw)


def producer_metadata(path: Path) -> tuple[int, int, int]:
    text = path.read_text(encoding="utf-8")
    start = text.index("data Main {") + len("data Main {")
    depth, end = 1, start
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    body = re.sub(r"//.*", "", text[start:end - 1])
    fields = 0
    for statement in body.split(";"):
        if ":" in statement:
            names = statement.rsplit("\n", 1)[-1].split(":", 1)[0]
            fields += sum(bool(item.strip()) for item in names.split(","))
    locals_used = len(re.findall(r"\blet\s+[A-Za-z_][A-Za-z0-9_]*\s*:", text))
    locals_used += sum(match.group(1).count(":") for match in re.finditer(
        r"\bstate\s+[A-Za-z_][A-Za-z0-9_]*\s*\(([^)]*)\)", text
    ))
    procedures = len(re.findall(r"^machine ", text, re.MULTILINE))
    return fields, locals_used, procedures


def inspect_positive(contents: bytes) -> ir5.Module:
    module = ir5.decode(contents)
    require(ir5.interpret(module) == 70, "independent CKIR5 result is not 70")
    operations = module.tables["operations"]
    opcodes = [row[3] for row in operations]
    require(14 in opcodes, "missing ConstructCase opcode 14")
    require(7 in opcodes, "missing structural Copy opcode 7")
    require(10 in opcodes, "missing structural Call opcode 10")

    cases = module.tables["cases"]
    require({row[4] for row in cases} >= {0, 1, 2, 3, 4}, "missing case arity 0..4")
    constructed_arities = {
        cases[row[10]][4] for row in operations if row[3] == 14
    }
    require(constructed_arities >= {0, 1, 2, 3, 4}, "missing construction arity 0..4")
    payloads = module.tables["case_payloads"]
    types = module.tables["types"]
    require(any(types[row[3]][1] == 4 for row in payloads), "missing nested record payload")

    dispatch_flags = {row[4] for row in module.tables["terminators"] if row[3] == 5}
    require(dispatch_flags >= {1, 2}, "missing value/place CaseDispatch pair")
    require(any(row[1] == 2 for row in module.tables["case_arm_args"]),
            "missing selected payload binding")
    require(any(offset > 0 for offset in module.field_offsets),
            "self-field dispatch did not use a nonzero field offset")
    return module


def run_gate(include_v6: bool = False) -> None:
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("resolved-to-CKIR5: skipped (requires Darwin arm64)")
        return

    repo = HERE.parents[2]
    resolver_source = COMPILER / "omega-bootstrap-resolve.alp"
    lowerer_source = COMPILER / "omega-bootstrap-resolved-to-ckir4.alp"
    lowermachine_source = repo / "bootstrap/rungs/delta/samples/lowermachine.alp"
    delta_manifest = repo / "bootstrap/onramps/delta-rust/Cargo.toml"
    delta = repo / "bootstrap/onramps/delta-rust/target/debug/delta"
    general_path = HERE / "fixtures/ckir5-payload-sums/general.omg"
    for path in (resolver_source, lowerer_source, lowermachine_source, general_path):
        require(path.is_file(), f"missing {path}")

    fields, locals_used, procedures = producer_metadata(lowerer_source)
    require(fields < 256 and locals_used <= 32 and procedures <= 128,
            f"lowermachine ceiling fields={fields} locals={locals_used} procedures={procedures}")

    timings: dict[str, float] = {}
    with tempfile.TemporaryDirectory(prefix="delta-resolved-to-ckir5-") as raw_temp:
        temp = Path(raw_temp)
        env = dict(os.environ, DELTA_ARCH="aarch64")

        def timed(name: str, command: list[str], **kwargs) -> subprocess.CompletedProcess[bytes]:
            begin = time.perf_counter()
            result = subprocess.run(command, **kwargs)
            timings[name] = time.perf_counter() - begin
            return result

        timed("cargo", ["cargo", "build", "-q", "--manifest-path", str(delta_manifest)], check=True)
        for name, source in (("resolver", resolver_source), ("lowerer", lowerer_source),
                             ("lowermachine", lowermachine_source)):
            timed(f"compile-{name}", [str(delta), str(source), str(temp / name)],
                  env=env, check=True, stdout=subprocess.DEVNULL)
        for name, source in (("resolver-self", resolver_source), ("lowerer-self", lowerer_source)):
            assembly = temp / f"{name}.s"
            with assembly.open("wb") as output:
                result = timed(
                    f"self-lower-{name}", [str(temp / "lowermachine")],
                    input=self_host_source(source), stdout=output,
                )
            require(result.returncode == 0, f"lowermachine rejected {name} source: {result.returncode}")
            timed(f"link-{name}", ["clang", "-arch", "arm64", "-o", str(temp / name), str(assembly)], check=True)
            subprocess.run(["codesign", "-f", "-s", "-", str(temp / name)], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def write_source(name: str, source: str) -> bytes:
            envelope = encode_source(source)
            (temp / f"{name}.omgc").write_bytes(envelope)
            return envelope

        def resolve(name: str, source: str, expected: int,
                    witness_major: int = 3) -> tuple[bytes, bytes]:
            envelope = write_source(name, source)
            results = [subprocess.run([str(temp / exe)], input=envelope, stdout=subprocess.PIPE)
                       for exe in ("resolver", "resolver-self")]
            statuses = [result.returncode for result in results]
            outputs = [result.stdout for result in results]
            require(statuses == [expected, expected],
                    f"{name} resolver native/self status {statuses}, expected {expected}")
            require(outputs[0] == outputs[1], f"{name} resolver native/self divergence")
            if expected:
                require(not outputs[0], f"{name} resolver published rejection bytes")
            else:
                require(outputs[0][:8] == f"OMGRSW{witness_major}".encode("ascii") + b"\0",
                        f"{name} did not select OMGRSW{witness_major}")
            return envelope, outputs[0]

        def lower(name: str, frame: bytes, expected: int) -> bytes:
            results = []
            for exe in ("lowerer", "lowerer-self"):
                begin = time.perf_counter()
                result = subprocess.run([str(temp / exe)], input=frame, stdout=subprocess.PIPE)
                timings[f"{exe}-{name}"] = time.perf_counter() - begin
                results.append(result)
            statuses = [result.returncode for result in results]
            outputs = [result.stdout for result in results]
            require(statuses == [expected, expected],
                    f"{name} lowerer native/self status {statuses}, expected {expected}")
            require(outputs[0] == outputs[1], f"{name} lowerer native/self byte divergence")
            if expected:
                require(not outputs[0], f"{name} lowerer published rejection bytes")
            return outputs[0]

        prepared: dict[str, tuple[bytes, bytes]] = {}
        for name, (source, status) in resource_sources().items():
            envelope, witness = resolve(name, source, status)
            if status == 0:
                prepared[name] = (envelope, witness)

        general = general_path.read_text(encoding="ascii")
        constructors = general[:general.index("machine SumProducer::consume")]
        constructors += """machine SumProducer::run(&mut self) -> u8 {
    self.current = Packet::Empty;
    self.current = Packet::One { value: 1 };
    self.current = Packet::Two { left: 1, right: 2 };
    self.current = Packet::Three { a: true, b: false, c: true };
    self.current = Packet::Four {
        a: 1, b: 2, nested: Pair { left: 3, right: 4 }, tail: 60
    };
    70
}
"""
        prepared["constructors"] = resolve("constructors", constructors, 0)
        prepared["general"] = resolve("general", general, 0)
        negative_prepared = {
            name: resolve(name, source, 0)
            for name, source in lowering_negatives(general).items()
        }

        legacy_source = """data Cell { value: u8; }
machine Cell::read(&self) -> u8 { self.value }
data SumProducer { cell: Cell; }
machine SumProducer::run(&mut self) -> u8 { self.cell.read() }
"""
        legacy_envelope = write_source("legacy-v2", legacy_source)
        legacy_results = [subprocess.run([str(temp / exe)], input=legacy_envelope, stdout=subprocess.PIPE)
                          for exe in ("resolver", "resolver-self")]
        require([result.returncode for result in legacy_results] == [0, 0], "legacy V2 resolution")
        require(legacy_results[0].stdout == legacy_results[1].stdout, "legacy V2 resolver divergence")
        require(legacy_results[0].stdout[:8] == b"OMGRSW2\0", "legacy source did not select V2")

        # Cross-pairs reject before any source lowering.
        lower("cross-v2-in-v6", pack_lowering(legacy_envelope, legacy_results[0].stdout, 6), 251)
        general_envelope, general_witness = prepared["general"]
        lower("cross-v3-in-v5", pack_lowering(general_envelope, general_witness, 5), 251)

        # Declaration-only witnesses prove the current shared metadata/layout/publication path.
        for name in ("declarations-single-payload", "cases-64",
                     "declarations-0-through-4"):
            envelope, witness = prepared[name]
            output = lower(name, pack_lowering(envelope, witness), 0)
            module = ir5.decode(output)
            require(ir5.interpret(module) == 70, f"{name} independent result")

        constructor_envelope, constructor_witness = prepared["constructors"]
        constructor_output = lower(
            "constructors", pack_lowering(constructor_envelope, constructor_witness), 0
        )
        constructor_module = ir5.decode(constructor_output)
        require(ir5.interpret(constructor_module) == 70, "constructors independent result")
        constructor_operations = constructor_module.tables["operations"]
        constructor_cases = constructor_module.tables["cases"]
        require({constructor_cases[row[10]][4] for row in constructor_operations if row[3] == 14}
                >= {0, 1, 2, 3, 4}, "constructor-only arity coverage")
        require(7 in {row[3] for row in constructor_operations},
                "constructor-only structural Copy")

        output = lower("general", pack_lowering(general_envelope, general_witness), 0)
        inspect_positive(output)
        for name, (envelope, witness) in negative_prepared.items():
            lower(name, pack_lowering(envelope, witness), 251)
        (temp / "general.ckir5").write_bytes(output)
        validated = subprocess.run(
            [sys.executable, "-B", str(HERE / "checked_ir_v5_reference.py"),
             "validate", str(temp / "general.ckir5")],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        require(validated.returncode == 0,
                f"independent CKIR5 validation failed: {validated.stderr.decode('utf-8', 'replace').strip()}")

        print(f"resolved-to-CKIR5: procedures={procedures}/128 fields={fields}/255 "
              f"locals={locals_used}/32 general={len(output)}B")
        print("resolved-to-CKIR5: OMGRSW3/OMGLOW6 native/self exact; declarations and 64-case "
              "controls; ConstructCase arities 0..4, nested payload, Copy/Call, value/place "
              "CaseDispatch, selected bindings, independent result=70; focused 251/252 passed")
        print("resolved-to-CKIR5 timings: " + " ".join(
            f"{name}={seconds:.3f}s" for name, seconds in sorted(timings.items())
            if name.startswith("compile-") or name.startswith("self-lower-")
            or name.endswith("-general")
        ))

        if include_v6:
            bool_sw1 = """data SumProducer { flag: bool; empty: bool; }
machine SumProducer::run(&mut self) -> u8 {
    self.flag = true;
    self.flag = !self.flag;
    self.empty = !self.flag;
    transition !!self.empty {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            bool_sw2 = """data Cell { flag: bool; }
machine Cell::read(&self, value: bool) -> bool { !(value) }
data SumProducer { cell: Cell; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.cell.read(false) {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            bool_sw3 = general + """
machine SumProducer::negated(&self) -> bool { !false }
"""

            def resolve_and_lower_v7(name: str, source: str, witness_major: int) -> bytes:
                envelope, witness = resolve(name, source, 0, witness_major)
                require(witness[8] == witness_major,
                        f"{name} selected OMGRSW{witness[8]}, expected {witness_major}")
                output6 = lower(name, pack_lowering(
                    envelope, witness, 7, witness_major
                ), 0)
                module6 = ir6.decode(output6)
                require(ir6.interpret(module6) == 70, f"{name} CKIR6 result")
                require(sum(row[3] == 15 for row in module6.tables["operations"]) > 0,
                        f"{name} missing LogicalNot")
                return output6

            outputs6 = {
                "logical-not-sw1": resolve_and_lower_v7("logical-not-sw1", bool_sw1, 1),
                "logical-not-sw2": resolve_and_lower_v7("logical-not-sw2", bool_sw2, 2),
                "logical-not-sw3": resolve_and_lower_v7("logical-not-sw3", bool_sw3, 3),
            }

            sw1_envelope, sw1_witness = resolve("logical-not-old-frame", bool_sw1, 0, 1)
            lower("logical-not-old-frame",
                  pack_lowering(sw1_envelope, sw1_witness, 4), 251)
            plain_sw1 = """data SumProducer {}
machine SumProducer::run(&mut self) -> u8 { 70 }
"""
            plain_envelope, plain_witness = resolve("logical-not-missing", plain_sw1, 0, 1)
            lower("logical-not-missing",
                  pack_lowering(plain_envelope, plain_witness, 7, 1), 251)
            lower("logical-not-selector-cross",
                  pack_lowering(sw1_envelope, sw1_witness, 7, 2), 251)

            # Literal depth one plus seven/eight prefixes selects exact total
            # expression depth 8/9.
            for count, expected in ((7, 0), (8, 252)):
                nested = bool_sw1.replace("!!self.empty", "!" * count + "false", 1)
                envelope, witness = resolve(f"logical-not-depth-{count}", nested, 0, 1)
                result = lower(f"logical-not-depth-{count}",
                               pack_lowering(envelope, witness, 7, 1), expected)
                if expected == 0:
                    require(ir6.interpret(ir6.decode(result)) == 70,
                            "logical-not expression-depth-8 result")

            for name, source in {
                "logical-not-integer": bool_sw1.replace("!self.flag", "!1", 1),
                "logical-not-dangling": bool_sw1.replace("!self.flag", "!;", 1),
                "logical-not-bitwise-bool": bool_sw1.replace("!self.flag", "~true", 1),
            }.items():
                envelope, witness = resolve(name, source, 0, 1)
                lower(name, pack_lowering(envelope, witness, 7, witness[8]), 251)

            print("resolved-to-CKIR6: OMGLOW7 independently pairs least OMGRSW1/2/3; "
                  "bool-only recursive LogicalNot, product field/call/sum composition, "
                  "native/self exact result 70; old/new cross-pairs and depth 8/9 passed; "
                  + " ".join(f"{name}={len(value)}B" for name, value in outputs6.items()))


def main() -> None:
    if len(sys.argv) != 2 or sys.argv[1] not in ("gate", "gate-v6"):
        raise ValueError("usage: delta-resolved-to-ckir5-fixture.py gate|gate-v6")
    run_gate(sys.argv[1] == "gate-v6")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, compilation.CompilationError, bundle.BundleError,
            ir5.Ckir5Error, ir6.Ckir6Error, subprocess.CalledProcessError) as error:
        raise SystemExit(f"resolved-to-CKIR5: {error}")
