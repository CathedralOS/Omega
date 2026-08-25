#!/usr/bin/env python3
"""Focused OMGLOW4/5 resolved-source to CKIR4 producer gate."""

from __future__ import annotations

import re
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "compiler"))
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402

PACKAGE = "33" * 32
NO_ID = 0xFFFFFFFF
WITNESS_TABLES = (
    ("units", 36), ("imports", 48), ("bindings", 28),
    ("declarations", 28), ("types", 24), ("records", 24),
    ("fields", 24), ("machines", 40), ("machine_parameters", 24),
    ("blocks", 40), ("block_parameters", 24),
)


def build(output: Path, owner: str, machine: str, sources: list[Path]) -> None:
    entries = [bundle.Entry(f"source-{index}.omg", source.read_bytes())
               for index, source in enumerate(sources)]
    packed = bundle.encode(entries)
    manifest = {
        "target": "linux_x86_64",
        "packages": [{"key": PACKAGE, "sources": [
            {"label": entry.label, "module": ""} for entry in entries]}],
        "aliases": [],
        "root": {"package": PACKAGE, "source": entries[-1].label,
                 "owner": owner, "machine": machine},
    }
    output.write_bytes(compilation.encode_manifest(manifest, packed))


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
    lets = len(re.findall(r"\blet\s+[A-Za-z_][A-Za-z0-9_]*\s*:", text))
    state_parameters = sum(match.group(1).count(":") for match in
                           re.finditer(r"\bstate\s+[A-Za-z_][A-Za-z0-9_]*\s*\(([^)]*)\)", text))
    return fields, lets + state_parameters, len(re.findall(r"^machine ", text, re.MULTILINE))


def inspect_ckir(raw: bytes) -> dict[str, object]:
    if len(raw) < 80 or raw[:8] != b"OMGCKIR\0":
        raise ValueError("CKIR4 magic/length")
    major, minor, target, flags = struct.unpack_from("<4H", raw, 8)
    if (major, minor, target) != (4, 0, 1) or flags & ~1:
        raise ValueError("CKIR4 fixed header")
    entry, length, *counts = struct.unpack_from("<16I", raw, 16)
    if length != len(raw):
        raise ValueError("CKIR4 exact length")
    (types, records, fields, machines, mparams, blocks, bparams,
     operations, operands, terms, values_count, places, constants, children) = counts
    expected = (80 + 24 * types + 20 * records + 16 * fields + 36 * machines
                + 20 * mparams + 32 * blocks + 20 * bparams + 24 * constants
                + 4 * children + 40 * operations + 4 * operands + 44 * terms)
    if expected != len(raw) or terms != blocks or (flags & 1 and entry == NO_ID):
        raise ValueError("CKIR4 table relation")
    at = (80 + 24 * types + 20 * records + 16 * fields + 36 * machines
          + 20 * mparams + 32 * blocks + 20 * bparams + 24 * constants + 4 * children)
    opcodes = [raw[at + 40 * index + 12] for index in range(operations)]
    return {"counts": tuple(counts), "operations": operations, "operands": operands,
            "values": values_count, "places": places, "opcodes": opcodes}


def assert_parameter_spans(envelope: bytes, witness: bytes, *,
                           machine_names: tuple[bytes, ...] = (),
                           block_names: tuple[bytes, ...] = ()) -> None:
    if witness[:8] not in (b"OMGRSW1\0", b"OMGRSW2\0") or len(witness) < 72:
        raise ValueError("parameter control witness header")
    counts = struct.unpack_from("<11I", witness, 20)
    offsets: dict[str, int] = {}
    at = 72
    for (name, stride), count in zip(WITNESS_TABLES, counts):
        offsets[name] = at; at += stride * count
    decoded = compilation.decode(envelope)
    sources = tuple(decoded.bundle_entries[row.bundle_entry_id].content for row in decoded.sources)

    def machine_source(machine: int) -> int:
        declaration = struct.unpack_from("<I", witness, offsets["machines"] + 40 * machine + 4)[0]
        return struct.unpack_from("<I", witness, offsets["declarations"] + 28 * declaration + 8)[0]

    observed_machine = []
    for index in range(counts[8]):
        row = struct.unpack_from("<6I", witness, offsets["machine_parameters"] + 24 * index)
        source = sources[machine_source(row[1])]
        observed_machine.append(source[row[4]:row[4] + row[5]])
    observed_block = []
    for index in range(counts[10]):
        row = struct.unpack_from("<6I", witness, offsets["block_parameters"] + 24 * index)
        machine = struct.unpack_from("<I", witness, offsets["blocks"] + 40 * row[1] + 4)[0]
        source = sources[machine_source(machine)]
        observed_block.append(source[row[4]:row[4] + row[5]])
    if any(name not in observed_machine for name in machine_names):
        raise ValueError(f"machine parameter name spans {observed_machine!r}")
    if any(name not in observed_block for name in block_names):
        raise ValueError(f"block parameter name spans {observed_block!r}")


def assert_role3_receivers(envelope: bytes, witness: bytes,
                           expected: tuple[tuple[bytes, bytes], ...]) -> None:
    counts = struct.unpack_from("<11I", witness, 20)
    offsets: dict[str, int] = {}
    at = 72
    for (name, stride), count in zip(WITNESS_TABLES, counts):
        offsets[name] = at; at += stride * count
    decoded = compilation.decode(envelope)
    sources = tuple(decoded.bundle_entries[row.bundle_entry_id].content for row in decoded.sources)

    def declaration_name(declaration: int) -> bytes:
        row = offsets["declarations"] + 28 * declaration
        source = struct.unpack_from("<I", witness, row + 8)[0]
        start, length = struct.unpack_from("<2I", witness, row + 16)
        return sources[source][start:start + length]

    observed = []
    for binding in range(counts[2]):
        row = offsets["bindings"] + 28 * binding
        if witness[row + 8] != 3:
            continue
        source = struct.unpack_from("<I", witness, row + 4)[0]
        start, length, declaration, import_id = struct.unpack_from("<4I", witness, row + 12)
        if witness[row + 9] != 2 or import_id != NO_ID:
            raise ValueError("field role-3 target kind/import")
        machine = struct.unpack_from("<I", witness, offsets["declarations"] + 28 * declaration + 24)[0]
        owner = struct.unpack_from("<I", witness, offsets["machines"] + 40 * machine + 8)[0]
        record_declaration = struct.unpack_from("<I", witness, offsets["records"] + 24 * owner + 4)[0]
        observed.append((sources[source][start:start + length], declaration_name(record_declaration)))
    if tuple(observed) != expected:
        raise ValueError(f"field role-3 receivers {observed!r}, expected {expected!r}")


def record_source(count: int, body: str | None = None) -> str:
    fields = "\n".join(f"    f{index}: u8;" for index in range(count))
    authored = body if body is not None else ", ".join(
        f"f{index}: {'self.scalar' if index == 0 else index}" for index in range(count))
    return f"""data ResourceValue [copy] {{
{fields}
}}
data ResourceProbe {{ value: ResourceValue; scalar: u8; }}
machine ResourceProbe::run(&mut self) -> u8 {{
    self.scalar = 70;
    self.value = ResourceValue {{ {authored} }};
    self.scalar
}}
"""


def self_host_source(path: Path) -> bytes:
    """Erase comments/indentation while preserving Delta tokens and strings."""
    raw = re.sub(rb"//[^\n]*", b"", path.read_bytes())
    return re.sub(rb"(?m)^[ \t]+", b"", raw)


def gate() -> None:
    import os
    import platform
    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("resolved-to-CKIR4: skipped (requires Darwin arm64)"); return
    repo = HERE.parents[2]
    producer = HERE.parent / "compiler" / "omega-bootstrap-resolved-to-ckir4.alp"
    resolver_source = HERE.parent / "compiler" / "omega-bootstrap-resolve.alp"
    frame_tool = HERE / "delta-resolved-to-ckir4-frame.py"
    reference = HERE / "checked_ir_v4_reference.py"
    lowermachine_source = repo / "bootstrap/rungs/delta/samples/lowermachine.alp"
    delta_manifest = repo / "bootstrap/onramps/delta-rust/Cargo.toml"
    delta = repo / "bootstrap/onramps/delta-rust/target/debug/delta"
    fixtures = HERE / "fixtures/ckir4-runtime-records"
    exact_source = repo / "compiler/psi/source/source.omg"
    fields, locals_used, procedures = producer_metadata(producer)
    if fields >= 256 or locals_used > 32 or procedures > 128:
        raise ValueError(f"lowermachine ceiling fields={fields} locals={locals_used} procedures={procedures}")

    timings: dict[str, float] = {}
    with tempfile.TemporaryDirectory(prefix="delta-resolved-to-ckir4-") as raw_temp:
        temp = Path(raw_temp); env = dict(os.environ, DELTA_ARCH="aarch64")

        def timed(name: str, command: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            begin = time.perf_counter(); result = subprocess.run(command, **kwargs)
            timings[name] = time.perf_counter() - begin; return result

        subprocess.run(["cargo", "build", "-q", "--manifest-path", str(delta_manifest)], check=True)
        for name, source in (("resolver", resolver_source), ("native", producer),
                             ("lowermachine", lowermachine_source)):
            result = timed(f"compile-{name}", [str(delta), str(source), str(temp / name)],
                           env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            if result.returncode:
                raise ValueError(f"Delta compile {name}: {result.stderr.decode(errors='replace')}")
        with (temp / "self.s").open("wb") as output:
            result = timed("lowermachine-source", [str(temp / "lowermachine")],
                           input=self_host_source(producer), stdout=output)
        if result.returncode:
            raise ValueError("lowermachine producer source")
        timed("clang-self", ["clang", "-arch", "arm64", "-o", str(temp / "self"),
                             str(temp / "self.s")], check=True)
        subprocess.run(["codesign", "-f", "-s", "-", str(temp / "self")], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        with (temp / "resolver-self.s").open("wb") as output:
            result = timed("lowermachine-resolver", [str(temp / "lowermachine")],
                           input=self_host_source(resolver_source), stdout=output)
        if result.returncode:
            raise ValueError("lowermachine resolver source")
        timed("clang-resolver-self", ["clang", "-arch", "arm64", "-o", str(temp / "resolver-self"),
                                      str(temp / "resolver-self.s")], check=True)
        subprocess.run(["codesign", "-f", "-s", "-", str(temp / "resolver-self")], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def prepare(name: str, owner: str, machine: str, sources: list[Path],
                    machine_names: tuple[bytes, ...] = (),
                    block_names: tuple[bytes, ...] = (), relation: int = 1,
                    role3_receivers: tuple[tuple[bytes, bytes], ...] = ()) -> bytes:
            envelope = temp / f"{name}.omgc"; build(envelope, owner, machine, sources)
            witness_outputs = []
            for resolver in (temp / "resolver", temp / "resolver-self"):
                witness_result = subprocess.run([str(resolver)], input=envelope.read_bytes(),
                                                stdout=subprocess.PIPE)
                if witness_result.returncode:
                    raise ValueError(f"{resolver.name} rejected {name}: {witness_result.returncode}")
                witness_outputs.append(witness_result.stdout)
            if witness_outputs[0] != witness_outputs[1]:
                raise ValueError(f"resolver native/self divergence for {name}")
            expected_witness = b"OMGRSW1\0" if relation == 1 else b"OMGRSW2\0"
            expected_frame = b"OMGLOW4\0" if relation == 1 else b"OMGLOW5\0"
            if witness_outputs[0][:8] != expected_witness:
                raise ValueError(f"{name} resolution relation")
            assert_parameter_spans(envelope.read_bytes(), witness_outputs[0],
                                   machine_names=machine_names, block_names=block_names)
            if role3_receivers:
                assert_role3_receivers(envelope.read_bytes(), witness_outputs[0], role3_receivers)
            witness = temp / f"{name}.omgrsw{relation}"; witness.write_bytes(witness_outputs[0])
            frame = subprocess.run([sys.executable, str(frame_tool), "pack", str(envelope), str(witness)],
                                   check=True, stdout=subprocess.PIPE).stdout
            if frame[:8] != expected_frame:
                raise ValueError(f"{name} lowering relation")
            return frame

        def expect(name: str, frame: bytes, status: int, result_value: int | None = None) -> bytes:
            outputs, statuses = [], []
            for kind in ("native", "self"):
                begin = time.perf_counter()
                result = subprocess.run([str(temp / kind)], input=frame, stdout=subprocess.PIPE)
                timings[f"{kind}-{name}"] = time.perf_counter() - begin
                statuses.append(result.returncode); outputs.append(result.stdout)
            if statuses != [status, status]:
                raise ValueError(f"{name} status native/self={statuses}, expected {status}")
            if outputs[0] != outputs[1]:
                raise ValueError(f"{name} native/self byte divergence")
            if status:
                if outputs[0]: raise ValueError(f"{name} published rejection bytes")
                return outputs[0]
            inspect_ckir(outputs[0])
            if result_value is not None:
                ckir = temp / f"{name}.ckir4"; ckir.write_bytes(outputs[0])
                observed = subprocess.run([sys.executable, "-B", str(reference), "run", str(ckir)],
                                          check=True, stdout=subprocess.PIPE).stdout.strip()
                if observed != str(result_value).encode("ascii"):
                    raise ValueError(f"{name} result {observed!r}, expected {result_value}")
            return outputs[0]

        def expect_resolver(name: str, owner: str, machine: str,
                            sources: list[Path], status: int) -> None:
            envelope = temp / f"{name}.omgc"; build(envelope, owner, machine, sources)
            outputs = []
            statuses = []
            for resolver in (temp / "resolver", temp / "resolver-self"):
                result = subprocess.run([str(resolver)], input=envelope.read_bytes(), stdout=subprocess.PIPE)
                statuses.append(result.returncode); outputs.append(result.stdout)
            if statuses != [status, status] or outputs[0] != outputs[1]:
                raise ValueError(f"{name} resolver native/self={statuses}")
            if status and outputs[0]:
                raise ValueError(f"{name} resolver published rejection bytes")

        positives = (
            ("authored", "RuntimePairProbe", "run", [fixtures / "authored-declaration-order.omg"], (), ()),
            ("reordered", "RuntimePairProbe", "run", [fixtures / "declaration-order.omg"], (), ()),
            ("constant", "ConstantPairProbe", "run", [fixtures / "constant-assignment.omg"], (), ()),
            ("nested", "NestedRuntimeProbe", "run", [fixtures / "nested-runtime.omg"], (b"value",), ()),
            ("direct", "DirectCallProbe", "run", [fixtures / "direct-call.omg"], (b"value",), ()),
            ("field-receiver", "FieldReceiverProbe", "run",
             [fixtures / "direct-field-receiver.omg"], (b"value",), (), 2,
             ((b"install", b"FieldCell"), (b"read", b"FieldCell"))),
            ("field-source-api", "SourceHost", "run",
             [exact_source, fixtures / "source-unit-field-harness.omg"], (), (), 2,
             ((b"clear", b"SourceUnit"), (b"append", b"SourceUnit"),
              (b"byte_or_nul", b"SourceUnit"))),
            ("exact-source", "SourceUnit", "bootstrap_runtime_record_probe",
             [exact_source, fixtures / "source-unit-harness.omg"], (), (b"runtime_scalar",)),
        )
        outputs: dict[str, bytes] = {}
        for positive in positives:
            name, owner, machine, sources, machine_names, block_names, *successor = positive
            relation = successor[0] if successor else 1
            role3_receivers = successor[1] if len(successor) > 1 else ()
            outputs[name] = expect(
                name,
                prepare(name, owner, machine, sources, machine_names, block_names,
                        relation, role3_receivers),
                0, 70,
            )
        if outputs["authored"] != outputs["reordered"]:
            raise ValueError("authored/declaration-order CKIR4 differs")
        for name in ("authored", "reordered", "nested", "direct", "exact-source"):
            if 13 not in inspect_ckir(outputs[name])["opcodes"]:
                raise ValueError(f"{name} omitted ConstructRecord opcode 13")
        constant_ops = inspect_ckir(outputs["constant"])["opcodes"]
        if 11 not in constant_ops or 13 in constant_ops:
            raise ValueError(f"constant assignment opcode path {constant_ops}")
        for field_name in ("field-receiver", "field-source-api"):
            field_ops = inspect_ckir(outputs[field_name])["opcodes"]
            if 10 not in field_ops or not any(field_ops[index:index + 2] == [2, 3]
                                              for index in range(len(field_ops) - 1)):
                raise ValueError(
                    f"{field_name} omitted SelfPlace/FieldPlace/Call chain {field_ops}"
                )
        direct_field_ops = inspect_ckir(outputs["field-receiver"])["opcodes"]
        if not any(direct_field_ops[index:index + 3] == [2, 3, 10]
                   for index in range(len(direct_field_ops) - 2)):
            raise ValueError("zero-argument field call lost direct operation adjacency")
        if 13 not in inspect_ckir(outputs["field-source-api"])["opcodes"]:
            raise ValueError("field SourceUnit API omitted runtime SourceId construction")

        # The two source relations are canonical, not interchangeable spellings.
        # Changing only the witness identity selects the paired frame identity,
        # then the lowerer must reject source that belongs to the other relation.
        for name, source_relation, target_relation in (
            ("field-downgrade", 2, 1), ("direct-upgrade", 1, 2),
        ):
            base = "field-receiver" if source_relation == 2 else "direct"
            witness = bytearray((temp / f"{base}.omgrsw{source_relation}").read_bytes())
            witness[6] = 48 + target_relation
            struct.pack_into("<H", witness, 8, target_relation)
            changed = temp / f"{name}.omgrsw{target_relation}"; changed.write_bytes(witness)
            frame = subprocess.run(
                [sys.executable, str(frame_tool), "pack", str(temp / f"{base}.omgc"), str(changed)],
                check=True, stdout=subprocess.PIPE,
            ).stdout
            expect(name, frame, 251)

        resolver_negative_sources = {
            "field-unknown": """data Cell { value: u8; }
data Probe { cell: Cell; }
machine Cell::read(&self) -> u8 { self.value }
machine Probe::run(&mut self) -> u8 { self.missing.read() }
""",
            "field-scalar": """data Probe { scalar: u8; }
machine Probe::run(&mut self) -> u8 { self.scalar.read() }
""",
            "field-wrong-owner": """data Cell { value: u8; }
data Other { value: u8; }
data Probe { cell: Cell; }
machine Other::read(&self) -> u8 { self.value }
machine Probe::run(&mut self) -> u8 { self.cell.read() }
""",
        }
        for name, text in resolver_negative_sources.items():
            source = temp / f"{name}.omg"; source.write_text(text, encoding="ascii")
            expect_resolver(name, "Probe", "run", [source], 251)

        lowering_negative_sources = {
            "field-shared-mutable": """data Cell { value: u8; }
data Probe { cell: Cell; }
machine Cell::clear(&mut self) { self.value = 0; }
machine Probe::run(&self) -> u8 { self.cell.clear(); 0 }
""",
            "field-parenthesized": """data Cell { value: u8; }
data Probe { cell: Cell; }
machine Cell::read(&self) -> u8 { self.value }
machine Probe::run(&mut self) -> u8 { (self.cell).read() }
""",
            "field-chained": """data Cell { value: u8; }
data Outer { cell: Cell; }
data Probe { outer: Outer; }
machine Cell::read(&self) -> u8 { self.value }
machine Probe::run(&mut self) -> u8 { self.outer.cell.read() }
""",
        }
        for name, text in lowering_negative_sources.items():
            source = temp / f"{name}.omg"; source.write_text(text, encoding="ascii")
            relation = 2 if name == "field-shared-mutable" else 1
            expect(name, prepare(name, "Probe", "run", [source], relation=relation), 251)

        generated: dict[str, tuple[str, int]] = {
            "record-4": (record_source(4), 0),
            "record-5": (record_source(5), 252),
            "five-duplicate": (record_source(5, "f0: self.scalar, f0: 1, f1: 1, f2: 2, f3: 3"), 251),
            "five-missing": (record_source(5, "f0: self.scalar, f1: 1, f2: 2, f3: 3"), 251),
            "five-unknown": (record_source(5, "f0: self.scalar, f1: 1, f2: 2, f3: 3, nope: 4"), 251),
            "five-mistyped": (record_source(5, "f0: self.scalar, f1: 1, f2: 2, f3: 3, f4: true"), 251),
            "unsupported-arithmetic": (record_source(4, "f0: self.scalar + 0, f1: 1, f2: 2, f3: 3"), 251),
            "unsupported-comparison": (record_source(4, "f0: self.scalar < 71, f1: 1, f2: 2, f3: 3"), 251),
            "named-field-load": (record_source(4, "f0: self.value.f0, f1: 1, f2: 2, f3: 3"), 0),
        }
        for name, (source_text, status) in generated.items():
            source = temp / f"{name}.omg"; source.write_text(source_text, encoding="ascii")
            output = expect(name, prepare(name, "ResourceProbe", "run", [source]), status,
                            70 if status == 0 else None)
            if status == 0 and 13 not in inspect_ckir(output)["opcodes"]:
                raise ValueError("record-4 did not publish ConstructRecord")

        missing_predecessor = prepare(
            "negative-index-missing-predecessor", "MissingBoundProbe", "run",
            [fixtures / "negative-index-missing-predecessor.omg"],
        )
        expect("negative-index-missing-predecessor", missing_predecessor, 251)

        print(f"resolved-to-CKIR4: procedures={procedures}/128 fields={fields}/255 "
              f"locals={locals_used}/32 exact-source={len(outputs['exact-source'])}B")
        print("resolved-to-CKIR4: runtime-record positives=8 native/self exact; "
              "OMGRSW1/2 and OMGLOW4/5 canonical/cross-pair controls; "
              "direct-field shape/owner/access negatives; record-4=0 record-5=252 "
              "semantic-before-5=251 missing-field-bound=251; result=70")
        print("resolved-to-CKIR4 timings: " + " ".join(
            f"{name}={seconds:.3f}s" for name, seconds in sorted(timings.items())
            if name.startswith("compile-") or name in ("lowermachine-source", "native-exact-source", "self-exact-source")))


def main(args: list[str]) -> int:
    if len(args) >= 5 and args[0] == "build":
        build(Path(args[1]), args[2], args[3], [Path(item) for item in args[4:]]); return 0
    if len(args) == 2 and args[0] == "inspect":
        print(inspect_ckir(Path(args[1]).read_bytes())); return 0
    if len(args) == 1 and args[0] == "gate":
        gate(); return 0
    raise ValueError("usage: build OUTPUT OWNER MACHINE SOURCE... | inspect CKIR4 | gate")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"delta-resolved-to-ckir4-fixture: {error}", file=sys.stderr)
        raise SystemExit(1)
