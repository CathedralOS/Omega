#!/usr/bin/env python3
"""Build single-package focused CKIR3 inputs and inspect producer output."""

from __future__ import annotations

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
FRAME_HEADER = struct.Struct("<8sHHHH4I")


def build(output: Path, owner: str, machine: str, sources: list[Path]) -> None:
    entries = [bundle.Entry(f"source-{i}.omg", path.read_bytes()) for i, path in enumerate(sources)]
    packed = bundle.encode(entries)
    manifest = {
        "target": "linux_x86_64",
        "packages": [{
            "key": PACKAGE,
            "sources": [{"label": entry.label, "module": ""} for entry in entries],
        }],
        "aliases": [],
        "root": {"package": PACKAGE, "source": entries[-1].label, "owner": owner, "machine": machine},
    }
    output.write_bytes(compilation.encode_manifest(manifest, packed))


def inspect_ckir(path: Path) -> str:
    raw = path.read_bytes()
    if len(raw) < 80 or raw[:8] != b"OMGCKIR\0":
        raise ValueError("CKIR3 magic/length")
    major, minor, target, flags = struct.unpack_from("<4H", raw, 8)
    if (major, minor, target) != (3, 0, 1) or flags & ~1:
        raise ValueError("CKIR3 fixed header")
    values = struct.unpack_from("<16I", raw, 16)
    entry, length = values[:2]
    counts = values[2:]
    if length != len(raw):
        raise ValueError("CKIR3 exact length")
    (types, records, fields, machines, mparams, blocks, bparams,
     operations, operands, terms, _values, _places, constants, children) = counts
    expected = (80 + 24 * types + 20 * records + 16 * fields + 36 * machines
                + 20 * mparams + 32 * blocks + 20 * bparams + 24 * constants
                + 4 * children + 40 * operations + 4 * operands + 44 * terms)
    if expected != len(raw) or terms != blocks or (flags & 1 and entry == NO_ID):
        raise ValueError("CKIR3 table length/relation")
    at = 80 + 24 * types + 20 * records + 16 * fields + 36 * machines + 20 * mparams + 32 * blocks + 20 * bparams
    nodes = [struct.unpack_from("<6I", raw, at + 24 * i) for i in range(constants)]
    at += 24 * constants
    child_ids = struct.unpack_from(f"<{children}I", raw, at) if children else ()
    at += 4 * children
    opcodes: list[int] = []
    roots: list[int] = []
    for i in range(operations):
        row = raw[at + 40 * i:at + 40 * (i + 1)]
        opcode = row[12]
        opcodes.append(opcode)
        if opcode == 11:
            roots.append(struct.unpack_from("<I", row, 32)[0])
    for index, (node_id, _type, start, count, _scalar, reserved) in enumerate(nodes):
        if node_id != index or reserved or start + count > children:
            raise ValueError("constant node row")
        if any(child >= index for child in child_ids[start:start + count]):
            raise ValueError("constant edge order")
    if any(root >= constants for root in roots):
        raise ValueError("opcode-11 root")
    return f"types={types} constants={constants} children={children} ops={operations} opcodes={','.join(map(str, sorted(set(opcodes))))} roots={len(roots)}"


def ckir_metrics(raw: bytes) -> dict[str, object]:
    values = struct.unpack_from("<16I", raw, 16)
    counts = values[2:]
    (types, records, fields, machines, mparams, blocks, bparams,
     operations, operands, _terms, _values, _places, constants, children) = counts
    at = 80 + 24 * types + 20 * records + 16 * fields + 36 * machines + 20 * mparams + 32 * blocks + 20 * bparams
    at += 24 * constants + 4 * children
    opcodes: list[int] = []
    roots: list[int] = []
    const_immediates: list[int] = []
    for index in range(operations):
        row = raw[at + 40 * index:at + 40 * (index + 1)]
        opcode = row[12]
        opcodes.append(opcode)
        immediate = struct.unpack_from("<I", row, 32)[0]
        if opcode == 1:
            const_immediates.append(immediate)
        if opcode == 11:
            roots.append(immediate)
    at += 40 * operations + 4 * operands
    term_kinds = [raw[at + 44 * index + 12] for index in range(blocks)]
    return {
        "constants": constants, "children": children, "operations": operations,
        "opcodes": opcodes, "roots": roots, "const_immediates": const_immediates,
        "term_kinds": term_kinds,
    }


def producer_metadata(path: Path) -> tuple[int, int]:
    """Mirror the two bounded metadata tables used by lowermachine.alp."""
    import re

    text = path.read_text(encoding="utf-8")
    start = text.index("data Main {") + len("data Main {")
    depth = 1
    end = start
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
    state_parameters = 0
    for match in re.finditer(r"\bstate\s+[A-Za-z_][A-Za-z0-9_]*\s*\(([^)]*)\)", text):
        state_parameters += match.group(1).count(":")
    return fields, lets + state_parameters


def resource_source(kind: str) -> str:
    if kind.startswith("array-"):
        count = int(kind.split("-", 1)[1])
        values = ", ".join(str(index & 255) for index in range(count))
        return f"""data ResourceProbe {{ values: [u8; {count}] in Trapping; }}
machine ResourceProbe::run(&mut self) -> u8 {{
    self.values = [{values}];
    70
}}
"""
    count = int(kind.split("-", 1)[1])
    fields = "\n".join(f"    f{index}: u8;" for index in range(count))
    values = ", ".join(f"f{index}: {index}" for index in range(count))
    return f"""data ResourceValue [copy] {{
{fields}
}}
data ResourceProbe {{ value: ResourceValue; }}
machine ResourceProbe::run(&mut self) -> u8 {{
    self.value = ResourceValue {{ {values} }};
    70
}}
"""


def replace_once(source: Path, output: Path, old: str, new: str) -> None:
    text = source.read_text(encoding="utf-8")
    if text.count(old) != 1:
        raise ValueError(f"source replacement anchor in {source.name}")
    output.write_text(text.replace(old, new), encoding="utf-8")


def padded_source(source: bytes, length: int) -> bytes:
    if len(source) > length:
        raise ValueError(f"source of {len(source)} bytes exceeds padding target {length}")
    return source + b" " * (length - len(source))


def append_final_source_space_unchecked(
    canonical: bytes, expected_length: int, expected_resource: str,
) -> bytes:
    """Construct one adjacent resource overage without weakening production encoding."""
    compilation.decode(canonical)
    header = compilation.HEADER.unpack_from(canonical)
    encoded_length, bundle_length, string_length = header[5:8]
    package_count, source_count, alias_count = header[9:12]
    if encoded_length != len(canonical) or source_count < 1:
        raise ValueError("unchecked source-adjacent base envelope")
    bundle_at = (compilation.HEADER.size + package_count * compilation.PACKAGE_ROW.size
                 + source_count * compilation.SOURCE_ROW.size
                 + alias_count * compilation.ALIAS_ROW.size + string_length)
    if len(canonical) - bundle_at != bundle_length:
        raise ValueError("unchecked source-adjacent bundle extent")
    magic, version, entry_count = bundle.HEADER.unpack_from(canonical, bundle_at)
    if (magic, version, entry_count) != (bundle.MAGIC, bundle.VERSION, source_count):
        raise ValueError("unchecked source-adjacent bundle header")
    cursor = bundle_at + bundle.HEADER.size
    final_header = -1
    final_content_length = -1
    for index in range(entry_count):
        entry_at = cursor
        label_length, content_length = bundle.ENTRY_HEADER.unpack_from(canonical, entry_at)
        cursor += bundle.ENTRY_HEADER.size + label_length + content_length
        if cursor > len(canonical):
            raise ValueError("unchecked source-adjacent entry extent")
        if index == entry_count - 1:
            final_header = entry_at
            final_content_length = content_length
    if cursor != len(canonical) or final_content_length != expected_length:
        raise ValueError("unchecked source-adjacent final entry")
    adjacent = bytearray(canonical)
    struct.pack_into("<I", adjacent, final_header + 4, final_content_length + 1)
    adjacent.append(32)
    struct.pack_into("<I", adjacent, 16, encoded_length + 1)
    struct.pack_into("<I", adjacent, 20, bundle_length + 1)
    try:
        compilation.decode(bytes(adjacent))
    except compilation.CompilationError as error:
        if error.status != 252 or str(error) != expected_resource:
            raise ValueError(
                f"unchecked source-adjacent result {error.status}: {error}"
            ) from error
    else:
        raise ValueError("unchecked source-adjacent envelope passed production limits")
    return bytes(adjacent)


def gate() -> None:
    import os
    import platform

    if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
        print("resolved-to-CKIR3: skipped (requires Darwin arm64)")
        return
    repo = HERE.parents[2]
    producer = HERE.parent / "compiler" / "omega-bootstrap-resolved-to-ckir3.alp"
    resolver_source = HERE.parent / "compiler" / "omega-bootstrap-resolve.alp"
    frame_tool = HERE / "delta-resolved-to-ckir3-frame.py"
    lowermachine_source = repo / "bootstrap/delta/samples/lowermachine.alp"
    delta_manifest = repo / "bootstrap/delta/rust/Cargo.toml"
    delta = repo / "bootstrap/delta/rust/target/debug/delta"
    fixtures = HERE / "fixtures/ckir3-constant-aggregates"
    reference = HERE / "checked_ir_v3_reference.py"
    unicode_source = repo / "source/compiler/omega/psi/generated/unicode_tables.omg"
    fields, locals_used = producer_metadata(producer)
    if fields >= 256 or locals_used > 32:
        raise ValueError(f"lowermachine metadata ceiling: fields={fields}, locals={locals_used}")

    timings: dict[str, float] = {}
    with tempfile.TemporaryDirectory(prefix="delta-resolved-to-ckir3-") as raw_temp:
        temp = Path(raw_temp)
        env = dict(os.environ, DELTA_ARCH="aarch64")

        def timed(name: str, command: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
            begin = time.perf_counter()
            result = subprocess.run(command, **kwargs)
            timings[name] = time.perf_counter() - begin
            return result

        subprocess.run(["cargo", "build", "-q", "--manifest-path", str(delta_manifest)], check=True)
        for name, source in (("resolver", resolver_source), ("native", producer), ("lowermachine", lowermachine_source)):
            result = timed(f"compile-{name}", [str(delta), str(source), str(temp / name)], env=env,
                           stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            if result.returncode:
                raise ValueError(f"Delta compile {name}: {result.stderr.decode(errors='replace')}")
        with (temp / "self.s").open("wb") as output:
            result = timed("lowermachine-source", [str(temp / "lowermachine")], input=producer.read_bytes(), stdout=output)
        if result.returncode:
            raise ValueError("lowermachine producer source")
        timed("clang-self", ["clang", "-arch", "arm64", "-o", str(temp / "self"), str(temp / "self.s")], check=True)
        subprocess.run(["codesign", "-f", "-s", "-", str(temp / "self")], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        def lower(executable: Path, frame: bytes) -> tuple[int, bytes, float]:
            begin = time.perf_counter()
            result = subprocess.run([str(executable)], input=frame, stdout=subprocess.PIPE)
            return result.returncode, result.stdout, time.perf_counter() - begin

        def frame_envelope(name: str, envelope: Path) -> bytes:
            witness = temp / f"{name}.witness"
            framed = temp / f"{name}.low3"
            result = subprocess.run([str(temp / "resolver")], input=envelope.read_bytes(), stdout=subprocess.PIPE)
            if result.returncode:
                raise ValueError(f"resolver rejected {name}: {result.returncode}")
            witness.write_bytes(result.stdout)
            with framed.open("wb") as output:
                subprocess.run([sys.executable, str(frame_tool), "pack", str(envelope), str(witness)],
                               check=True, stdout=output)
            return framed.read_bytes()

        def prepare(name: str, owner: str, machine: str, sources: list[Path]) -> bytes:
            envelope = temp / f"{name}.omgc"
            build(envelope, owner, machine, sources)
            return frame_envelope(name, envelope)

        def prepare_unchecked(name: str, envelope_bytes: bytes) -> bytes:
            envelope = temp / f"{name}.omgc"
            envelope.write_bytes(envelope_bytes)
            return frame_envelope(name, envelope)

        def expect(name: str, frame: bytes, status: int) -> bytes:
            native_status, native_output, native_time = lower(temp / "native", frame)
            self_status, self_output, self_time = lower(temp / "self", frame)
            timings[f"native-{name}"] = native_time
            timings[f"self-{name}"] = self_time
            if (native_status, self_status) != (status, status):
                raise ValueError(f"{name} status native/self={native_status}/{self_status}, expected {status}")
            if native_output != self_output:
                raise ValueError(f"{name} native/self byte divergence")
            if status and native_output:
                raise ValueError(f"{name} published bytes on rejection")
            return native_output

        def expect_result(name: str, output: bytes, expected: int) -> None:
            path = temp / f"{name}.ckir3"
            path.write_bytes(output)
            inspect_ckir(path)
            result = subprocess.run(
                [sys.executable, "-B", str(reference), "run", str(path)],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            observed = result.stdout.decode("ascii", errors="replace").strip()
            if result.returncode or observed != str(expected):
                detail = result.stderr.decode(errors="replace")[-1000:]
                raise ValueError(
                    f"{name} independent result status={result.returncode} "
                    f"value={observed!r}, expected {expected}: {detail}"
                )

        authored_true = temp / "authored-true-transition.omg"
        replace_once(
            fixtures / "guardless-transition.omg", authored_true,
            "    transition { _ -> pass() }",
            "    transition true {\n        true -> pass()\n        _ -> pass()\n    }",
        )
        arm_no_fact = temp / "arm-local-edge-argument-no-fact.omg"
        replace_once(
            fixtures / "arm-local-edge-argument.omg", arm_no_fact,
            "            true -> accept(self.values[index])\n            _ -> fail()",
            "            true -> accept(70)\n            _ -> accept(self.values[index])",
        )
        missing_cycle_reordered = temp / "negative-missing-cycle-predecessor-reordered.omg"
        replace_once(
            fixtures / "negative-missing-cycle-predecessor.omg", missing_cycle_reordered,
            """    state inspect(&mut self, index: u32 in Trapping) {
        transition self.values[index] < 255 {
            true -> back()
            _ -> fail()
        }
    }

    state back(&mut self) {
        transition { _ -> inspect(2) }
    }""",
            """    state back(&mut self) {
        transition { _ -> inspect(2) }
    }

    state inspect(&mut self, index: u32 in Trapping) {
        transition self.values[index] < 255 {
            true -> back()
            _ -> fail()
        }
    }""",
        )

        guardless_source = (fixtures / "guardless-transition.omg").read_bytes()
        source_unit_exact = temp / "source-unit-131072.omg"
        source_unit_exact.write_bytes(padded_source(guardless_source, 131_072))
        aggregate_support = temp / "aggregate-source-support.omg"
        aggregate_space = temp / "aggregate-source-space.omg"
        aggregate_root = temp / "aggregate-source-root.omg"
        aggregate_support.write_bytes(padded_source(b"data AggregatePadding {}\n", 131_072))
        aggregate_space.write_bytes(b" " * 65_536)
        aggregate_root.write_bytes(padded_source(guardless_source, 65_536))
        aggregate_sources = [aggregate_support, aggregate_space, aggregate_root]
        if sum(path.stat().st_size for path in aggregate_sources) != 262_144:
            raise ValueError("aggregate source exact fixture length")

        positives = [
            ("guardless", "GuardlessProbe", "run", [fixtures / "guardless-transition.omg"]),
            ("authored-true", "GuardlessProbe", "run", [authored_true]),
            ("arm-local-edge", "ArmArgumentProbe", "run", [fixtures / "arm-local-edge-argument.omg"]),
            ("stale-parameter", "StaleIdentityProbe", "run", [fixtures / "stale-parameter-custody.omg"]),
            ("cyclic", "CustodyCycle", "run", [fixtures / "cyclic-range-custody.omg"]),
            ("renamed", "AggregateProbe", "run", [fixtures / "renamed-reordered-nested.omg"]),
            ("unicode", "UnicodeTables", "bootstrap_constant_aggregate_probe",
             [unicode_source, fixtures / "unicode-harness.omg"]),
            ("source-unit-131072", "GuardlessProbe", "run", [source_unit_exact]),
            ("aggregate-source-262144", "GuardlessProbe", "run", aggregate_sources),
        ]
        outputs: dict[str, bytes] = {}
        frames: dict[str, bytes] = {}
        for name, owner, machine, sources in positives:
            frames[name] = prepare(name, owner, machine, sources)
            outputs[name] = expect(name, frames[name], 0)
            expect_result(name, outputs[name], 70)
        metrics = {name: ckir_metrics(output) for name, output in outputs.items()}
        guardless = metrics["guardless"]
        if (guardless["constants"], guardless["children"], guardless["operations"]) != (0, 0, 1):
            raise ValueError(f"guardless operation shape {guardless}")
        if guardless["opcodes"] != [1] or guardless["const_immediates"] != [70] or guardless["term_kinds"] != [1, 4]:
            raise ValueError(f"guardless must be Jump plus authored result Const(70), got {guardless}")
        authored = metrics["authored-true"]
        if (authored["constants"], authored["children"], authored["operations"]) != (0, 0, 2):
            raise ValueError(f"authored-true operation shape {authored}")
        if (authored["opcodes"] != [1, 1] or authored["const_immediates"] != [1, 70]
                or authored["term_kinds"] != [2, 4]):
            raise ValueError(f"authored true must be Branch plus Const(1) and Const(70), got {authored}")
        if outputs["guardless"] == outputs["authored-true"]:
            raise ValueError("guardless Jump and authored-true Branch publications are not distinct")
        cyclic = metrics["cyclic"]
        if (cyclic["constants"], cyclic["children"], cyclic["operations"]) != (8, 9, 30):
            raise ValueError(f"cyclic exact counts {cyclic}")
        if 11 not in cyclic["opcodes"] or 12 not in cyclic["opcodes"] or 1 not in cyclic["term_kinds"]:
            raise ValueError(f"cyclic CKIR3 feature coverage {cyclic}")
        renamed = metrics["renamed"]
        if (renamed["constants"], renamed["children"], renamed["operations"]) != (28, 28, 102):
            raise ValueError(f"renamed exact counts {renamed}")
        unicode = metrics["unicode"]
        if (unicode["constants"], unicode["children"], unicode["operations"], len(unicode["roots"])) != (2740, 3537, 244, 2):
            raise ValueError(f"Unicode exact CKIR3 counts {unicode}")
        if 11 not in unicode["opcodes"] or 12 not in unicode["opcodes"]:
            raise ValueError(f"Unicode CKIR3 opcode coverage {unicode}")

        source_unit_over = append_final_source_space_unchecked(
            (temp / "source-unit-131072.omgc").read_bytes(), 131_072,
            "source content bytes exceeds 131072",
        )
        expect(
            "source-unit-131073",
            prepare_unchecked("source-unit-131073", source_unit_over),
            252,
        )
        aggregate_source_over = append_final_source_space_unchecked(
            (temp / "aggregate-source-262144.omgc").read_bytes(), 65_536,
            "aggregate source content bytes exceeds 262144",
        )
        expect(
            "aggregate-source-262145",
            prepare_unchecked("aggregate-source-262145", aggregate_source_over),
            252,
        )

        source_negatives = (
            ("negative-wrong-field", "WrongFieldProbe", 251),
            ("negative-wrong-type", "WrongTypeProbe", 251),
            ("negative-array-arity", "WrongArityProbe", 251),
            ("negative-nonconstant-member", "RuntimeMemberProbe", 251),
            ("negative-noncopy-aggregate", "NoncopyAggregateProbe", 251),
            ("negative-less-equal-type", "LessEqualTypeProbe", 251),
            ("negative-dynamic-index-no-fact", "UnsafeIndexProbe", 251),
            ("negative-missing-field", "MissingFieldProbe", 251),
            ("negative-duplicate-field", "DuplicateFieldProbe", 251),
            ("negative-unknown-extra-field", "UnknownExtraFieldProbe", 251),
            ("negative-nominal-mismatch", "NominalMismatchProbe", 251),
            ("negative-out-of-range-u8", "OutOfRangeProbe", 251),
            ("negative-noncopy-constant-root", "NoncopyRootProbe", 251),
            ("negative-shared-place-mutation", "SharedMutationProbe", 251),
            ("negative-recursive-layout", "RecursiveLayoutProbe", 251),
            ("negative-oversized-layout", "OversizedLayoutProbe", 252),
            ("negative-less-equal-bool", "BoolLessEqualProbe", 251),
            ("negative-less-equal-structural", "StructuralLessEqualProbe", 251),
            ("negative-less-equal-mixed-carrier", "MixedCarrierProbe", 251),
            ("negative-less-equal-missing-rhs", "MissingRhsProbe", 251),
            ("negative-less-equal-chain", "ChainedLessEqualProbe", 251),
        )
        for fixture, owner, status in source_negatives:
            frame = prepare(fixture, owner, "run", [fixtures / f"{fixture}.omg"])
            expect(fixture, frame, status)

        interval_negatives = (
            ("arm-local-edge-no-fact", "ArmArgumentProbe", arm_no_fact),
            ("missing-cycle-predecessor", "MissingCycleProbe",
             fixtures / "negative-missing-cycle-predecessor.omg"),
            ("missing-cycle-predecessor-reordered", "MissingCycleProbe", missing_cycle_reordered),
        )
        for name, owner, source in interval_negatives:
            frame = prepare(name, owner, "run", [source])
            expect(name, frame, 251)

        for kind, status in (("array-1024", 0), ("array-1025", 252), ("record-4", 0), ("record-5", 252)):
            source = temp / f"{kind}.omg"
            source.write_text(resource_source(kind), encoding="ascii")
            frame = prepare(kind, "ResourceProbe", "run", [source])
            expect(kind, frame, status)

        wrong_identity = bytearray(frames["guardless"])
        wrong_identity[6] = ord("2")
        expect("wrong-OMGLOW-identity", bytes(wrong_identity), 251)
        wrong_major = bytearray(frames["guardless"])
        wrong_major[8] = 2
        expect("wrong-OMGLOW-major", bytes(wrong_major), 251)
        for name, comp, witness in (("comp-cap", 267_281, 0), ("witness-cap", 0, 524_289)):
            total = FRAME_HEADER.size + comp + witness
            frame = FRAME_HEADER.pack(b"OMGLOW3\0", 3, 0, 0, FRAME_HEADER.size, total, comp, witness, 0)
            frame += bytes(comp + witness)
            expect(name, frame, 252)

        print(f"resolved-to-CKIR3: fields={fields}/255 locals={locals_used}/32 "
              f"unicode={len(outputs['unicode'])}B nodes=2740 children=3537")
        semantic_251 = sum(status == 251 for _, _, status in source_negatives)
        semantic_252 = sum(status == 252 for _, _, status in source_negatives)
        print("resolved-to-CKIR3 controls: array-1024=0 array-1025=252 record-4=0 record-5=252 "
              "source-unit-131072=0 source-unit-131073=252 "
              "aggregate-source-262144=0 aggregate-source-262145=252 "
              f"comp-cap=252 witness-cap=252 source-251={semantic_251} source-252={semantic_252} "
              "interval-custody-251=3 wrong-identity=251 wrong-major=251")
        print("resolved-to-CKIR3 timings: " + " ".join(
            f"{name}={seconds:.3f}s" for name, seconds in sorted(timings.items())
            if (name.startswith("compile-") or name == "lowermachine-source" or name.endswith("-unicode")
                or "source-unit-" in name or "aggregate-source-" in name)
        ))


def main(args: list[str]) -> int:
    if len(args) >= 5 and args[0] == "build":
        build(Path(args[1]), args[2], args[3], [Path(item) for item in args[4:]])
        return 0
    if len(args) == 2 and args[0] == "inspect":
        print(inspect_ckir(Path(args[1])))
        return 0
    if len(args) == 1 and args[0] == "gate":
        gate()
        return 0
    raise ValueError("usage: build OUTPUT OWNER MACHINE SOURCE... | inspect CKIR3 | gate")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as error:
        print(f"delta-resolved-to-ckir3-fixture: {error}", file=sys.stderr)
        raise SystemExit(2)
