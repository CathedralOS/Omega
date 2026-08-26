#!/usr/bin/env python3
"""Shared focused OMGLOW6-through-C source-to-CKIR5-through-11 gate."""

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
try:
    import checked_ir_v7_reference as ir7  # noqa: E402
except ImportError:  # The CKIR7 sibling lands with the focused backend tranche.
    ir7 = None
try:
    import checked_ir_v8_reference as ir8  # noqa: E402
except ImportError:
    ir8 = None
try:
    import checked_ir_v9_reference as ir9  # noqa: E402
except ImportError:
    ir9 = None
try:
    import checked_ir_v10_reference as ir10  # noqa: E402
except ImportError:
    ir10 = None
try:
    import checked_ir_v11_reference as ir11  # noqa: E402
except ImportError:
    ir11 = None
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


PACKAGE = "55" * 32
LOWER_HEADER = struct.Struct("<8sHHHH4I")
GATE_ERRORS = (
    OSError, ValueError, compilation.CompilationError, bundle.BundleError,
    ir5.Ckir5Error, ir6.Ckir6Error, subprocess.CalledProcessError,
) + (() if ir7 is None else (ir7.Ckir7Error,)) \
    + (() if ir8 is None else (ir8.Ckir8Error,)) \
    + (() if ir9 is None else (ir9.Ckir9Error,)) \
    + (() if ir10 is None else (ir10.Ckir10Error,)) \
    + (() if ir11 is None else (ir11.Ckir11Error,))


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
    require(version in (4, 5, 6, 7, 8, 9, 10, 11, 12), "test lowering version")
    require((version in (7, 8, 9, 10, 11, 12) and resolution in (1, 2, 3))
            or (version not in (7, 8, 9, 10, 11, 12) and resolution == 0), "test resolution selector")
    total = LOWER_HEADER.size + len(comp) + len(witness)
    require(total <= 791_600, "OMGLOW frame capacity")
    magic = (b"OMGLOWC\0" if version == 12 else b"OMGLOWB\0" if version == 11 else b"OMGLOWA\0" if version == 10
             else f"OMGLOW{version}".encode("ascii") + b"\0")
    return LOWER_HEADER.pack(
        magic, version, 0, 0,
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


def run_gate(mode: str = "v5") -> None:
    include_v6 = mode in ("v6", "v7", "v8", "v9", "v10", "v11")
    include_v7 = mode in ("v7", "v8", "v9", "v10", "v11")
    include_v8 = mode in ("v8", "v9", "v10", "v11")
    include_v9 = mode in ("v9", "v10", "v11")
    include_v10 = mode in ("v10", "v11")
    include_v11 = mode == "v11"
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("resolved-to-CKIR5: skipped (requires Darwin arm64)")
        return

    repo = HERE.parents[2]
    resolver_source = COMPILER / "omega-bootstrap-resolve.alp"
    lowerer_source = COMPILER / "omega-bootstrap-resolved-to-ckir4.alp"
    lowermachine_source = repo / "bootstrap/delta/samples/lowermachine.alp"
    delta_manifest = repo / "bootstrap/delta/rust/Cargo.toml"
    delta = repo / "bootstrap/delta/rust/target/debug/delta"
    general_path = HERE / "fixtures/ckir5-payload-sums/general.omg"
    equality_general_path = HERE / "fixtures/ckir8-scalar-equality/general.omg"
    greater_general_path = HERE / "fixtures/ckir9-ordered-comparison/general.omg"
    widen_general_path = HERE / "fixtures/ckir10-integer-widen/general.omg"
    trapping_add_general_path = HERE / "fixtures/ckir11-trapping-add/general.omg"
    for path in (resolver_source, lowerer_source, lowermachine_source, general_path,
                 equality_general_path, greater_general_path, widen_general_path,
                 trapping_add_general_path):
        require(path.is_file(), f"missing {path}")

    fields, locals_used, procedures = producer_metadata(lowerer_source)
    require(fields < 512 and locals_used <= 32 and procedures <= 128,
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

        print(f"resolved-to-CKIR5: procedures={procedures}/128 fields={fields}/511 "
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

        if include_v7:
            require(ir7 is not None, "missing CKIR7 independent reference")
            bool_binary_sw1 = """data SumProducer { flag: bool; }
machine SumProducer::run(&mut self) -> u8 {
    self.flag = true;
    transition self.flag || true && false {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            bool_binary_sw2 = """data Cell { flag: bool; }
machine Cell::read(&self) -> bool { self.flag }
data SumProducer { cell: Cell; flag: bool; }
machine SumProducer::run(&mut self) -> u8 {
    self.flag = self.cell.read();
    self.flag = true;
    transition self.flag || true && false {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            bool_binary_sw3 = general + """
machine SumProducer::logical_binary(&self) -> bool {
    true || true && false
}
"""

            def resolve_and_lower_v8(name: str, source: str, witness_major: int) -> bytes:
                envelope, witness = resolve(name, source, 0, witness_major)
                require(witness[8] == witness_major,
                        f"{name} selected OMGRSW{witness[8]}, expected {witness_major}")
                output7 = lower(name, pack_lowering(
                    envelope, witness, 8, witness_major
                ), 0)
                module7 = ir7.decode(output7)
                require(ir7.interpret(module7) == 70, f"{name} CKIR7 result")
                opcodes = [row[3] for row in module7.tables["operations"]]
                require(16 in opcodes and 17 in opcodes,
                        f"{name} missing LogicalAnd/LogicalOr")
                require(opcodes.count(16) == source.count("&&")
                        and opcodes.count(17) == source.count("||"),
                        f"{name} logical token/operation correspondence")
                require(opcodes.index(16) < opcodes.index(17),
                        f"{name} did not lower && before ||")
                return output7

            outputs7 = {
                "logical-binary-sw1": resolve_and_lower_v8(
                    "logical-binary-sw1", bool_binary_sw1, 1
                ),
                "logical-binary-sw2": resolve_and_lower_v8(
                    "logical-binary-sw2", bool_binary_sw2, 2
                ),
                "logical-binary-sw3": resolve_and_lower_v8(
                    "logical-binary-sw3", bool_binary_sw3, 3
                ),
            }

            association = """data SumProducer {}
machine SumProducer::run(&mut self) -> u8 {
    transition true && true && true { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            association_envelope, association_witness = resolve(
                "logical-binary-association", association, 0, 1
            )
            association_output = lower(
                "logical-binary-association",
                pack_lowering(association_envelope, association_witness, 8, 1), 0,
            )
            association_module = ir7.decode(association_output)
            association_ops = [
                row for row in association_module.tables["operations"] if row[3] == 16
            ]
            require(len(association_ops) == 2, "logical association operation count")
            second_left = association_module.tables["operands"][association_ops[1][8]][0]
            require(second_left == association_ops[0][6],
                    "logical && chain is not left-associated")
            require(ir7.interpret(association_module) == 70,
                    "logical association result")

            sw1_envelope, sw1_witness = resolve(
                "logical-binary-old-frame", bool_binary_sw1, 0, 1
            )
            lower("logical-binary-old-frame",
                  pack_lowering(sw1_envelope, sw1_witness, 7, 1), 251)
            not_only = bool_binary_sw1.replace(
                "self.flag || true && false", "!!self.flag", 1
            )
            not_envelope, not_witness = resolve("logical-binary-missing", not_only, 0, 1)
            lower("logical-binary-missing",
                  pack_lowering(not_envelope, not_witness, 8, 1), 251)
            lower("logical-binary-selector-cross",
                  pack_lowering(sw1_envelope, sw1_witness, 8, 2), 251)

            # Literal depth one, six/seven prefixes, and one binary node select
            # exact total expression depth 8/9.
            for count, expected in ((6, 0), (7, 252)):
                nested = bool_binary_sw1.replace(
                    "self.flag || true && false", "!" * count + "true && true", 1
                )
                envelope, witness = resolve(f"logical-binary-depth-{count}", nested, 0, 1)
                result = lower(f"logical-binary-depth-{count}",
                               pack_lowering(envelope, witness, 8, 1), expected)
                if expected == 0:
                    require(ir7.interpret(ir7.decode(result)) == 70,
                            "logical-binary expression-depth-8 result")

            call_operand = """data SumProducer {}
machine SumProducer::probe(&self) -> bool { true }
machine SumProducer::run(&mut self) -> u8 {
    transition true || self.probe() { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            index_operand = """data SumProducer { flags: [bool; 1]; }
machine SumProducer::run(&mut self) -> u8 {
    transition true || self.flags[0] { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            trapping_operand = bool_binary_sw1.replace(
                "self.flag || true && false", "true || 1 + 1 < 3", 1
            )
            for name, source in {
                "logical-binary-non-bool": bool_binary_sw1.replace(
                    "self.flag || true && false", "true && 1", 1
                ),
                "logical-binary-single-and": bool_binary_sw1.replace(
                    "self.flag || true && false", "true & false", 1
                ),
                "logical-binary-single-or": bool_binary_sw1.replace(
                    "self.flag || true && false", "true | false", 1
                ),
                "logical-binary-call-operand": call_operand,
                "logical-binary-index-operand": index_operand,
                "logical-binary-trapping-operand": trapping_operand,
            }.items():
                envelope, witness = resolve(name, source, 0, 1)
                lower(name, pack_lowering(envelope, witness, 8, witness[8]), 251)

            print("resolved-to-CKIR7: OMGLOW8 independently pairs least OMGRSW1/2/3; "
                  "pure/nontrapping bool-only &&/|| with && precedence, inherited !/sum/call "
                  "composition, native/self exact result 70; purity, old/new, selector, and "
                  "depth 8/9 controls passed; "
                  + " ".join(f"{name}={len(value)}B" for name, value in outputs7.items()))

        if include_v8:
            require(ir8 is not None, "missing CKIR8 independent reference")
            equality_sw1 = """data SumProducer { word: u32; byte: u8; flag: bool; }
machine SumProducer::run(&mut self) -> u8 {
    self.word = 70;
    self.byte = 70;
    self.flag = true;
    transition self.word == 70 && self.byte == 70 && self.flag == true {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            equality_sw2 = """data Cell { word: u32; }
machine Cell::read(&self) -> u32 { self.word }
data SumProducer { cell: Cell; word: u32; }
machine SumProducer::run(&mut self) -> u8 {
    self.word = self.cell.read();
    self.word = 70;
    transition self.word == 70 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            equality_sw3 = equality_general_path.read_text(encoding="ascii")

            def resolve_and_lower_v9(name: str, source: str, witness_major: int) -> bytes:
                envelope, witness = resolve(name, source, 0, witness_major)
                require(witness[8] == witness_major,
                        f"{name} selected OMGRSW{witness[8]}, expected {witness_major}")
                output8 = lower(name, pack_lowering(
                    envelope, witness, 9, witness_major
                ), 0)
                module8 = ir8.decode(output8)
                require(ir8.interpret(module8) == 70, f"{name} CKIR8 result")
                opcodes = [row[3] for row in module8.tables["operations"]]
                require(opcodes.count(18) == source.count("=="),
                        f"{name} equality token/operation correspondence")
                return output8

            outputs8 = {
                "scalar-equal-sw1": resolve_and_lower_v9(
                    "scalar-equal-sw1", equality_sw1, 1
                ),
                "scalar-equal-sw2": resolve_and_lower_v9(
                    "scalar-equal-sw2", equality_sw2, 2
                ),
                "scalar-equal-sw3": resolve_and_lower_v9(
                    "scalar-equal-sw3", equality_sw3, 3
                ),
            }

            association = """data SumProducer {}
machine SumProducer::run(&mut self) -> u8 {
    transition true == false == false { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            association_envelope, association_witness = resolve(
                "scalar-equal-association", association, 0, 1
            )
            association_output = lower(
                "scalar-equal-association",
                pack_lowering(association_envelope, association_witness, 9, 1), 0,
            )
            association_module = ir8.decode(association_output)
            association_ops = [
                row for row in association_module.tables["operations"] if row[3] == 18
            ]
            require(len(association_ops) == 2, "scalar equality association operation count")
            second_left = association_module.tables["operands"][association_ops[1][8]][0]
            require(second_left == association_ops[0][6],
                    "scalar equality chain is not left-associated")
            require(ir8.interpret(association_module) == 70,
                    "scalar equality association result")

            precedence = """data SumProducer {}
machine SumProducer::run(&mut self) -> u8 {
    transition 1 < 2 == true && true || false { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            precedence_envelope, precedence_witness = resolve(
                "scalar-equal-precedence", precedence, 0, 1
            )
            precedence_output = lower(
                "scalar-equal-precedence",
                pack_lowering(precedence_envelope, precedence_witness, 9, 1), 0,
            )
            precedence_module = ir8.decode(precedence_output)
            selected = [
                row[3] for row in precedence_module.tables["operations"]
                if row[3] in (9, 16, 17, 18)
            ]
            require(selected == [9, 18, 16, 17],
                    f"scalar equality precedence operation order {selected}")
            require(ir8.interpret(precedence_module) == 70,
                    "scalar equality precedence result")

            sw1_envelope, sw1_witness = resolve(
                "scalar-equal-old-frame", equality_sw1, 0, 1
            )
            lower("scalar-equal-old-frame",
                  pack_lowering(sw1_envelope, sw1_witness, 8, 1), 251)
            inherited_only = equality_sw1.replace(
                "self.word == 70 && self.byte == 70 && self.flag == true",
                "!!self.flag && true", 1,
            )
            inherited_envelope, inherited_witness = resolve(
                "scalar-equal-missing", inherited_only, 0, 1
            )
            lower("scalar-equal-missing",
                  pack_lowering(inherited_envelope, inherited_witness, 9, 1), 251)
            lower("scalar-equal-selector-cross",
                  pack_lowering(sw1_envelope, sw1_witness, 9, 2), 251)

            for count, expected in ((7, 0), (8, 252)):
                expression = "true" + " == true" * count
                nested = association.replace("true == false == false", expression, 1)
                envelope, witness = resolve(f"scalar-equal-depth-{count}", nested, 0, 1)
                result = lower(f"scalar-equal-depth-{count}",
                               pack_lowering(envelope, witness, 9, 1), expected)
                if expected == 0:
                    require(ir8.interpret(ir8.decode(result)) == 70,
                            "scalar equality expression-depth-8 result")

            call_operand = """data SumProducer {}
machine SumProducer::probe(&self) -> u32 { 70 }
machine SumProducer::run(&mut self) -> u8 {
    transition self.probe() == 70 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            index_operand = """data SumProducer { words: [u32; 1]; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.words[0] == 70 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            mixed_carrier = """data SumProducer { byte: u8; word: u32; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.byte == self.word { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            record_operand = """data Pair [copy] { value: u32; }
data SumProducer { pair: Pair; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.pair == self.pair { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            sum_operand = """data Base [copy] { case A; case B; }
data SumProducer { base: Base; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.base == Base::A { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            for name, (source, witness_major) in {
                "scalar-equal-bool-numeric": (association.replace(
                    "true == false == false", "true == 1", 1
                ), 1),
                "scalar-equal-mixed-carrier": (mixed_carrier, 1),
                "scalar-equal-record": (record_operand, 1),
                "scalar-equal-sum": (sum_operand, 3),
                "scalar-equal-not-equal": (association.replace(
                    "true == false == false", "true != false", 1
                ), 1),
                "scalar-equal-missing-rhs": (association.replace(
                    "true == false == false", "true ==", 1
                ), 1),
                "scalar-equal-call-operand": (call_operand, 1),
                "scalar-equal-index-operand": (index_operand, 1),
                "scalar-equal-trapping-operand": (association.replace(
                    "true == false == false", "1 + 1 == 2", 1
                ), 1),
            }.items():
                envelope, witness = resolve(name, source, 0, witness_major)
                lower(name, pack_lowering(envelope, witness, 9, witness[8]), 251)

            print("resolved-to-CKIR8: OMGLOW9 independently pairs least OMGRSW1/2/3; "
                  "pure/nontrapping same-carrier bool/u8/u32 ScalarEqual with ordering/equality/"
                  "logical precedence and left association, inherited logical/sum/call composition, "
                  "native/self exact result 70; type/purity/old-new/selector/depth controls passed; "
                  + " ".join(f"{name}={len(value)}B" for name, value in outputs8.items()))

        if include_v9:
            require(ir9 is not None, "missing CKIR9 independent reference")
            greater_sw1 = """data SumProducer { word: u32; byte: u8; }
machine SumProducer::run(&mut self) -> u8 {
    self.word = 70;
    self.byte = 70;
    transition self.word > 69 && self.byte >= 70 {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            greater_sw2 = """data Cell { word: u32; }
machine Cell::read(&self) -> u32 { self.word }
data SumProducer { cell: Cell; word: u32; }
machine SumProducer::run(&mut self) -> u8 {
    self.word = self.cell.read();
    self.word = 70;
    transition self.word >= 70 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            greater_sw3 = greater_general_path.read_text(encoding="ascii")

            def resolve_and_lower_v10(name: str, source: str,
                                      witness_major: int) -> bytes:
                envelope, witness = resolve(name, source, 0, witness_major)
                require(witness[8] == witness_major,
                        f"{name} selected OMGRSW{witness[8]}, expected {witness_major}")
                output9 = lower(name, pack_lowering(
                    envelope, witness, 10, witness_major
                ), 0)
                module9 = ir9.decode(output9)
                require(ir9.interpret(module9) == 70, f"{name} CKIR9 result")
                opcodes = [row[3] for row in module9.tables["operations"]]
                authored_greater = len(re.findall(r"(?<!-)>(?!=)", source))
                authored_greater_equal = len(re.findall(r"(?<!-)>=", source))
                require(opcodes.count(19) == authored_greater,
                        f"{name} Greater token/operation correspondence")
                require(opcodes.count(20) == authored_greater_equal,
                        f"{name} GreaterEqual token/operation correspondence")
                return output9

            outputs9 = {
                "ordered-greater-sw1": resolve_and_lower_v10(
                    "ordered-greater-sw1", greater_sw1, 1
                ),
                "ordered-greater-sw2": resolve_and_lower_v10(
                    "ordered-greater-sw2", greater_sw2, 2
                ),
                "ordered-greater-sw3": resolve_and_lower_v10(
                    "ordered-greater-sw3", greater_sw3, 3
                ),
            }

            precedence = """data SumProducer {}
machine SumProducer::run(&mut self) -> u8 {
    transition 3 > 2 == true && 2 >= 2 || false {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            precedence_envelope, precedence_witness = resolve(
                "ordered-greater-precedence", precedence, 0, 1
            )
            precedence_output = lower(
                "ordered-greater-precedence",
                pack_lowering(precedence_envelope, precedence_witness, 10, 1), 0,
            )
            precedence_module = ir9.decode(precedence_output)
            selected = [
                row[3] for row in precedence_module.tables["operations"]
                if row[3] in (16, 17, 18, 19, 20)
            ]
            require(selected == [19, 18, 20, 16, 17],
                    f"ordered greater precedence operation order {selected}")
            greater_row = next(
                row for row in precedence_module.tables["operations"] if row[3] == 19
            )
            greater_operands = precedence_module.tables["operands"][
                greater_row[8]:greater_row[8] + greater_row[9]
            ]
            producers = {
                row[6]: row for row in precedence_module.tables["operations"]
                if row[4] == 1
            }
            require([producers[item[0]][10] for item in greater_operands] == [3, 2],
                    "ordered Greater did not preserve authored left/right order")
            require(ir9.interpret(precedence_module) == 70,
                    "ordered greater precedence result")

            sw1_envelope, sw1_witness = resolve(
                "ordered-greater-old-frame", greater_sw1, 0, 1
            )
            lower("ordered-greater-old-frame",
                  pack_lowering(sw1_envelope, sw1_witness, 9, 1), 251)
            inherited_only = greater_sw1.replace(
                "self.word > 69 && self.byte >= 70",
                "self.word == 70 && true", 1,
            )
            inherited_envelope, inherited_witness = resolve(
                "ordered-greater-missing", inherited_only, 0, 1
            )
            lower("ordered-greater-missing",
                  pack_lowering(inherited_envelope, inherited_witness, 10, 1), 251)
            lower("ordered-greater-selector-cross",
                  pack_lowering(sw1_envelope, sw1_witness, 10, 2), 251)

            depth_base = precedence.replace(
                "3 > 2 == true && 2 >= 2 || false", "9 > 8", 1
            )
            for equal_count, expected in ((6, 0), (7, 252)):
                expression = "9 > 8" + " == true" * equal_count
                nested = depth_base.replace("9 > 8", expression, 1)
                envelope, witness = resolve(
                    f"ordered-greater-depth-{equal_count}", nested, 0, 1
                )
                result = lower(
                    f"ordered-greater-depth-{equal_count}",
                    pack_lowering(envelope, witness, 10, 1), expected,
                )
                if expected == 0:
                    require(ir9.interpret(ir9.decode(result)) == 70,
                            "ordered greater expression-depth-8 result")

            call_operand = """data SumProducer {}
machine SumProducer::probe(&self) -> u32 { 70 }
machine SumProducer::run(&mut self) -> u8 {
    transition self.probe() > 69 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            index_operand = """data SumProducer { words: [u32; 1]; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.words[0] > 69 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            mixed_carrier = """data SumProducer { byte: u8; word: u32; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.byte > self.word { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            record_operand = """data Pair [copy] { value: u32; }
data SumProducer { pair: Pair; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.pair > self.pair { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            sum_operand = """data Base [copy] { case A; case B; }
data SumProducer { base: Base; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.base > Base::A { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            guard_fact_leak = """data SumProducer { index: u32; words: [u8; 1]; }
machine SumProducer::run(&mut self) -> u8 {
    transition self.index > 1 { true -> indexed() false -> failed() }
    state indexed(&mut self) { self.words[self.index] }
    state failed(&mut self) { 70 }
}
"""
            for name, (source, witness_major) in {
                "ordered-greater-bool": (precedence.replace(
                    "3 > 2 == true && 2 >= 2 || false", "true > false", 1
                ), 1),
                "ordered-greater-mixed-carrier": (mixed_carrier, 1),
                "ordered-greater-record": (record_operand, 1),
                "ordered-greater-sum": (sum_operand, 3),
                "ordered-greater-missing-rhs": (precedence.replace(
                    "3 > 2 == true && 2 >= 2 || false", "1 >", 1
                ), 1),
                "ordered-greater-left-associated-chain": (precedence.replace(
                    "3 > 2 == true && 2 >= 2 || false", "3 > 2 > 1", 1
                ), 1),
                "ordered-greater-call-operand": (call_operand, 1),
                "ordered-greater-index-operand": (index_operand, 1),
                "ordered-greater-trapping-operand": (precedence.replace(
                    "3 > 2 == true && 2 >= 2 || false", "1 + 1 > 1", 1
                ), 1),
                "ordered-greater-no-upper-fact": (guard_fact_leak, 1),
            }.items():
                envelope, witness = resolve(name, source, 0, witness_major)
                lower(name, pack_lowering(envelope, witness, 10, witness[8]), 251)

            print("resolved-to-CKIR9: OMGLOWA independently pairs least OMGRSW1/2/3; "
                  "pure/nontrapping same-carrier u8/u32 Greater/GreaterEqual with authored "
                  "operand order, ordering/equality/logical precedence, inherited equality/"
                  "logical/sum/call composition, and no upper-bound guard fact; native/self "
                  "exact result 70; type/purity/old-new/selector/depth controls passed; "
                  + " ".join(f"{name}={len(value)}B" for name, value in outputs9.items()))

        if include_v10:
            require(ir10 is not None, "missing CKIR10 independent reference")
            widen_sw1 = """data SumProducer { byte: u8; wide: u32 in Trapping; }
machine SumProducer::run(&mut self) -> u8 {
    self.byte = 0;
    self.wide = self.byte as u32 in Trapping;
    self.byte = 70;
    self.wide = (self.byte) as u32 in Trapping;
    self.byte = 255;
    self.wide = self.byte as u32 in Trapping;
    transition self.wide == 255 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            widen_sw2 = """data Cell {}
machine Cell::keep(&self, value: u32 in Trapping) -> u32 in Trapping { value }
data SumProducer { cell: Cell; byte: u8; wide: u32 in Trapping; }
machine SumProducer::run(&mut self) -> u8 {
    self.byte = 70;
    self.wide = self.cell.keep((self.byte) as u32 in Trapping);
    transition self.wide == 70 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            widen_sw3 = widen_general_path.read_text(encoding="ascii")

            def resolve_and_lower_v11(name: str, source: str,
                                      witness_major: int) -> bytes:
                envelope, witness = resolve(name, source, 0, witness_major)
                require(witness[8] == witness_major,
                        f"{name} selected OMGRSW{witness[8]}, expected {witness_major}")
                output10 = lower(name, pack_lowering(
                    envelope, witness, 11, witness_major
                ), 0)
                module10 = ir10.decode(output10)
                require(ir10.interpret(module10) == 70, f"{name} CKIR10 result")
                authored = source.count(" as u32 in Trapping")
                actual = sum(row[3] == 21 for row in module10.tables["operations"])
                require(actual == authored,
                        f"{name} IntegerWiden token/operation correspondence {actual}/{authored}")
                return output10

            outputs10 = {
                "integer-widen-sw1": resolve_and_lower_v11(
                    "integer-widen-sw1", widen_sw1, 1
                ),
                "integer-widen-sw2": resolve_and_lower_v11(
                    "integer-widen-sw2", widen_sw2, 2
                ),
                "integer-widen-sw3": resolve_and_lower_v11(
                    "integer-widen-sw3", widen_sw3, 3
                ),
            }

            sw1_envelope, sw1_witness = resolve(
                "integer-widen-old-frame", widen_sw1, 0, 1
            )
            lower("integer-widen-old-frame",
                  pack_lowering(sw1_envelope, sw1_witness, 10, 1), 251)
            inherited_only = widen_sw1.replace(
                "self.wide = self.byte as u32 in Trapping;",
                "self.wide = 0;", 1,
            ).replace(
                "self.wide = (self.byte) as u32 in Trapping;",
                "self.wide = 70;", 1,
            ).replace(
                "self.wide = self.byte as u32 in Trapping;",
                "self.wide = 255;", 1,
            )
            inherited_envelope, inherited_witness = resolve(
                "integer-widen-missing", inherited_only, 0, 1
            )
            lower("integer-widen-missing",
                  pack_lowering(inherited_envelope, inherited_witness, 11, 1), 251)
            lower("integer-widen-selector-cross",
                  pack_lowering(sw1_envelope, sw1_witness, 11, 2), 251)

            depth_base = """data SumProducer { byte: u8; wide: u32 in Trapping; }
machine SumProducer::run(&mut self) -> u8 {
    self.wide = self.byte as u32 in Trapping;
    transition DEPTH_EXPRESSION { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            for prefix_count, expected in ((7, 0), (8, 252)):
                expression = "!" * prefix_count + "false"
                nested = depth_base.replace("DEPTH_EXPRESSION", expression, 1)
                envelope, witness = resolve(
                    f"integer-widen-depth-{prefix_count}", nested, 0, 1
                )
                output = lower(
                    f"integer-widen-depth-{prefix_count}",
                    pack_lowering(envelope, witness, 11, 1), expected,
                )
                if expected == 0:
                    require(ir10.interpret(ir10.decode(output)) == 70,
                            "IntegerWiden expression-depth-8 result")

            negatives = {
                "integer-widen-narrowing": (widen_sw1.replace(
                    "as u32 in Trapping", "as u8 in Trapping", 1), 1),
                "integer-widen-bare": (widen_sw1.replace(
                    "as u32 in Trapping", "as u32", 1), 1),
                "integer-widen-source-u32": (widen_sw1.replace(
                    "self.byte as u32 in Trapping", "self.wide as u32 in Trapping", 1), 1),
                "integer-widen-source-bool": (widen_sw1.replace(
                    "self.byte as u32 in Trapping", "true as u32 in Trapping", 1), 1),
                "integer-widen-target-u64": (widen_sw1.replace(
                    "as u32 in Trapping", "as u64 in Trapping", 1), 1),
                "integer-widen-target-i32": (widen_sw1.replace(
                    "as u32 in Trapping", "as i32 in Trapping", 1), 1),
                "integer-widen-wrapping": (widen_sw1.replace(
                    "as u32 in Trapping", "as u32 in Wrapping", 1), 1),
                "integer-widen-saturating": (widen_sw1.replace(
                    "as u32 in Trapping", "as u32 in Saturating", 1), 1),
                "integer-widen-call-operand": (widen_sw2.replace(
                    "(self.byte) as u32 in Trapping", "self.cell.keep(0) as u32 in Trapping", 1), 2),
                "integer-widen-arithmetic-operand": (widen_sw1.replace(
                    "self.byte as u32 in Trapping", "(self.byte + 1) as u32 in Trapping", 1), 1),
                "integer-widen-index-operand": ("""data SumProducer { bytes: [u8; 1]; wide: u32 in Trapping; }
machine SumProducer::run(&mut self) -> u8 {
    self.wide = self.bytes[0] as u32 in Trapping;
    70
}
""", 1),
                "integer-widen-structural-operand": ("""data Pair [copy] { byte: u8; }
data SumProducer { pair: Pair; wide: u32 in Trapping; }
machine SumProducer::run(&mut self) -> u8 {
    self.wide = self.pair as u32 in Trapping;
    70
}
""", 1),
            }
            for name, (source, witness_major) in negatives.items():
                envelope, witness = resolve(name, source, 0, witness_major)
                lower(name, pack_lowering(envelope, witness, 11, witness[8]), 251)

            print("resolved-to-CKIR10: OMGLOWB independently pairs least OMGRSW1/2/3; "
                  "pure/total/nontrapping exact-u8 leaf `as u32 in Trapping` in parenthesized, "
                  "assignment, and single-argument call contexts; 0/70/255 payload preservation, "
                  "native/self exact result 70; old/new, selector, source/target/policy/purity/depth "
                  "controls passed; "
                  + " ".join(f"{name}={len(value)}B" for name, value in outputs10.items()))

        if include_v11:
            require(ir11 is not None, "missing CKIR11 independent reference")
            add_sw1 = """data SumProducer { cursor: u32 in Trapping; }
machine SumProducer::run(&mut self) -> u8 {
    self.cursor = 68;
    self.cursor = self.cursor + 1;
    transition (self.cursor) + 1 < 71 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            add_sw2 = """data Gate {}
machine Gate::keep(&self, prefix: u8, value: u32 in Trapping, suffix: u8) -> u32 in Trapping { value }
data SumProducer { gate: Gate; cursor: u32 in Trapping; }
machine SumProducer::run(&mut self) -> u8 {
    self.cursor = 69;
    self.cursor = self.gate.keep(1, self.cursor + 1, 2);
    transition self.cursor == 70 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
"""
            add_sw3 = trapping_add_general_path.read_text(encoding="ascii")

            def authored_selected_adds(source: str) -> int:
                return len(re.findall(
                    r"(?:\bself\.[A-Za-z_][A-Za-z0-9_]*|\b[A-Za-z_][A-Za-z0-9_]*)"
                    r"\s*\)?\s*\+\s*[0-9]+\b", source,
                ))

            def resolve_and_lower_v12(name: str, source: str,
                                      witness_major: int,
                                      *, run: bool = True) -> bytes:
                envelope, witness = resolve(name, source, 0, witness_major)
                require(witness[8] == witness_major,
                        f"{name} selected OMGRSW{witness[8]}, expected {witness_major}")
                output11 = lower(name, pack_lowering(
                    envelope, witness, 12, witness_major
                ), 0)
                module11 = ir11.decode(output11)
                if run:
                    require(ir11.interpret(module11) == 70, f"{name} CKIR11 result")
                authored = authored_selected_adds(source)
                actual = ir11.selected_add_count(module11)
                require(authored > 0 and actual == authored,
                        f"{name} trapping Add token/operation correspondence {actual}/{authored}")
                return output11

            outputs11 = {
                "trapping-add-sw1": resolve_and_lower_v12(
                    "trapping-add-sw1", add_sw1, 1
                ),
                "trapping-add-sw2": resolve_and_lower_v12(
                    "trapping-add-sw2", add_sw2, 2
                ),
                "trapping-add-sw3": resolve_and_lower_v12(
                    "trapping-add-sw3", add_sw3, 3
                ),
            }

            sw1_envelope, sw1_witness = resolve(
                "trapping-add-old-frame", add_sw1, 0, 1
            )
            lower("trapping-add-old-frame",
                  pack_lowering(sw1_envelope, sw1_witness, 11, 1), 251)
            inherited_only = add_sw1.replace(
                "self.cursor = self.cursor + 1;", "self.cursor = 69;", 1
            ).replace(
                "(self.cursor) + 1 < 71", "self.cursor < 71", 1
            )
            inherited_envelope, inherited_witness = resolve(
                "trapping-add-missing", inherited_only, 0, 1
            )
            lower("trapping-add-missing",
                  pack_lowering(inherited_envelope, inherited_witness, 12, 1), 251)
            lower("trapping-add-selector-cross",
                  pack_lowering(sw1_envelope, sw1_witness, 12, 2), 251)

            overflow_possible = """data SumProducer { cursor: u32 in Trapping; }
machine SumProducer::run(&mut self) -> u8 {
    self.cursor = self.cursor + 1;
    70
}
"""
            resolve_and_lower_v12(
                "trapping-add-runtime-overflow", overflow_possible, 1, run=False
            )

            two_trapping_args = add_sw2.replace(
                "machine Gate::keep(&self, prefix: u8, value: u32 in Trapping, suffix: u8) -> u32 in Trapping { value }",
                "machine Gate::keep(&self, prefix: u32 in Trapping, value: u32 in Trapping, suffix: u8) -> u32 in Trapping { value }",
                1,
            ).replace(
                "self.gate.keep(1, self.cursor + 1, 2)",
                "self.gate.keep(self.cursor + 1, self.cursor + 2, 2)", 1,
            )
            negatives = {
                "trapping-add-u8-left": add_sw1.replace(
                    "cursor: u32 in Trapping", "cursor: u8", 1
                ),
                "trapping-add-literal-left": inherited_only.replace(
                    "self.cursor < 71", "1 + self.cursor < 71", 1
                ),
                "trapping-add-typed-right": inherited_only.replace(
                    "self.cursor < 71", "self.cursor + self.cursor < 71", 1
                ),
                "trapping-add-call-left": add_sw2.replace(
                    "self.cursor + 1", "self.gate.keep(1, self.cursor, 2) + 1", 1
                ),
                "trapping-add-two-trapping-call-args": two_trapping_args,
            }
            for name, source in negatives.items():
                witness_major = 3 if "transition" in name else 2 if "call" in name else 1
                envelope, witness = resolve(name, source, 0, witness_major)
                lower(name, pack_lowering(envelope, witness, 12, witness[8]), 251)

            print("resolved-to-CKIR11: OMGLOWC independently pairs least OMGRSW1/2/3; "
                  "canonical u32-in-Trapping leaf-plus-literal Add in assignment, guard, "
                  "multi-argument call, and multi-argument transition contexts; one-potentially-"
                  "trapping-call-argument order rule; runtime-overflow source remains admitted; native/"
                  "self exact result 70; old/new, selector, carrier/shape/order controls passed; "
                  + " ".join(f"{name}={len(value)}B" for name, value in outputs11.items()))


def main() -> None:
    if len(sys.argv) != 2 or sys.argv[1] not in (
            "gate", "gate-v6", "gate-v7", "gate-v8", "gate-v9", "gate-v10", "gate-v11"):
        raise ValueError(
            "usage: delta-resolved-to-ckir5-fixture.py gate|gate-v6|gate-v7|gate-v8|gate-v9|gate-v10|gate-v11"
        )
    run_gate({
        "gate": "v5", "gate-v6": "v6", "gate-v7": "v7",
        "gate-v8": "v8", "gate-v9": "v9", "gate-v10": "v10",
        "gate-v11": "v11",
    }[sys.argv[1]])


if __name__ == "__main__":
    try:
        main()
    except GATE_ERRORS as error:
        raise SystemExit(f"resolved-to-CKIR5: {error}")
