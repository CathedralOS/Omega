#!/usr/bin/env python3
"""Generate and check the greatest source-realizable OMGLOW3 frame fixture."""

from __future__ import annotations

import os
import signal
import struct
import subprocess
import sys
import time
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "compiler"))
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402

from resolution_handoff_reference import decode_witness  # noqa: E402


SOURCE_BYTES = 262_144
COMPILATION_BYTES = 267_224
WITNESS_BYTES = 461_424
FRAME_BYTES = 728_680
ADJACENT_FRAME_BYTES = 728_681
COUNTS = (16, 64, 4096, 256, 2048, 128, 4096, 128, 889, 2048, 3206)


def sources() -> list[bytes]:
    modules = ([f"m{i}" for i in range(8)]
               + [f"m{i:02d}" + "z" * 61 for i in range(8, 15)]
               + ["m15" + "z" * 29])
    aliases = list("abcdefgh") + [f"p{i:02d}" + "x" * 61 for i in range(24)]
    require(sum(map(len, modules)) + sum(map(len, aliases)) + 8 == 2048,
            "canonical string payload construction")

    next_range = 1
    unique_block_types = 900
    block_parameters = 3206
    state_index = 0
    result: list[bytes] = []
    nonterminals = [(package, local) for package in range(16) for local in range(1, 8)]
    nominal_records = set(nonterminals[:62])
    scalar_records = set(nonterminals[62:64])
    selected_machine = 120

    for package in range(16):
        parts: list[str] = []
        if package == 15:
            for record in range(64):
                target = record // 8
                parts.append(f"use {aliases[target]}::{modules[target]}::D{record};")

        for local in range(8):
            record = package * 8 + local
            fields: list[str] = []
            if (package, local) in nominal_records:
                fields = [f"f{index}:D{package * 8};" for index in range(64)]
            elif (package, local) in scalar_records:
                for index in range(64):
                    fields.append(f"f{index}:u32 [0..={next_range}];")
                    next_range += 1
            parts.append(f"pub data D{record}{{{''.join(fields)}}}")

        for local in range(8):
            machine = package * 8 + local
            machine_parameters: list[str] = []
            if machine != selected_machine:
                for index in range(7):
                    machine_parameters.append(f"p{index}:u32 [0..={next_range}]")
                    next_range += 1

            states: list[str] = []
            for ordinal in range(15):
                remaining_states = 1920 - state_index
                count = min(2, block_parameters)
                if block_parameters > 2 * (remaining_states - 1):
                    count = block_parameters - 2 * (remaining_states - 1)
                parameters: list[str] = []
                for index in range(count):
                    if unique_block_types:
                        parameter_type = f"u32 [0..={next_range}]"
                        next_range += 1
                        unique_block_types -= 1
                    else:
                        parameter_type = "u8"
                    parameters.append(f"q{index}:{parameter_type}")
                body = "70" if machine == selected_machine else ""
                suffix = "," + ",".join(parameters) if parameters else ""
                states.append(f"state s{ordinal}(&mut self{suffix}){{{body}}}")
                block_parameters -= count
                state_index += 1

            parameters = "," + ",".join(machine_parameters) if machine_parameters else ""
            result_type = "->u8" if machine == selected_machine else ""
            body = "70" if machine == selected_machine else ""
            parts.append(
                f"machine D{package * 8}::m{machine}(&mut self{parameters})"
                f"{result_type}{{{body}{''.join(states)}}}"
            )
        result.append("".join(parts).encode("ascii"))

    require((sum(map(len, result)), next_range - 1, unique_block_types,
             block_parameters, state_index) == (125_065, 1917, 0, 0, 1920),
            "generated source census")
    return result


def metadata() -> tuple[list[str], list[str], list[bytes]]:
    modules = ([f"m{i}" for i in range(8)]
               + [f"m{i:02d}" + "z" * 61 for i in range(8, 15)]
               + ["m15" + "z" * 29])
    aliases = list("abcdefgh") + [f"p{i:02d}" + "x" * 61 for i in range(24)]
    keys = [bytes([index + 1]) * 32 for index in range(16)]
    return modules, aliases, keys


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    raw_sources = sources()
    padded = list(raw_sources)
    remaining = 131_072
    padded[0] = raw_sources[0] + b" " * (131_072 - len(raw_sources[0]))
    for index in range(1, 16):
        later = sum(map(len, raw_sources[index + 1:]))
        target = min(131_072, remaining - later)
        require(target >= len(raw_sources[index]), "source padding allocation")
        padded[index] = raw_sources[index] + b" " * (target - len(raw_sources[index]))
        remaining -= target
    require(sum(map(len, padded)) == SOURCE_BYTES and len(padded[-1]) < 131_072,
            "exact aggregate source padding")

    labels = [f"{index:02d}" + "l" * 58 + ".omg" for index in range(16)]
    require(all(len(label) == 64 for label in labels), "exact source labels")
    packed = bundle.encode([
        bundle.Entry(label, content) for label, content in zip(labels, padded)
    ])
    modules, aliases, keys = metadata()
    packages = [{
        "key": key.hex(),
        "sources": [{"label": labels[index], "module": modules[index]}],
    } for index, key in enumerate(keys)]
    alias_rows = []
    for index, name in enumerate(aliases):
        target = index if index < 15 else 0
        alias_rows.append({
            "requester": keys[15].hex(), "alias": name, "target": keys[target].hex(),
        })
    manifest = {
        "target": "linux_x86_64",
        "packages": packages,
        "aliases": alias_rows,
        "root": {
            "package": keys[15].hex(), "source": labels[15],
            "owner": "D120", "machine": "m120",
        },
    }
    exact = compilation.encode_manifest(manifest, packed)
    decoded = compilation.decode(exact)
    require(len(exact) == COMPILATION_BYTES, "exact OMGCOMP byte length")
    require(len(decoded.packages) == len(decoded.sources) == 16, "package/source maxima")
    require(len(decoded.aliases) == 32 and len(decoded.strings) == 50, "alias/string maxima")
    require(sum(len(value) for value in decoded.strings) == 2048, "string payload maximum")
    require(sum(len(entry.label) for entry in decoded.bundle_entries) == 1024,
            "label payload maximum")
    require(sum(len(entry.content) for entry in decoded.bundle_entries) == SOURCE_BYTES,
            "source payload maximum")
    (output / "exact.omgc").write_bytes(exact)
    (output / "adjacent.omgc").write_bytes(adjacent(exact))


def adjacent(exact: bytes) -> bytes:
    decoded = compilation.decode(exact)
    require(len(exact) == COMPILATION_BYTES, "adjacent exact base")
    header = compilation.HEADER.unpack_from(exact)
    bundle_at = len(exact) - header[6]
    magic, version, count = bundle.HEADER.unpack_from(exact, bundle_at)
    require((magic, version, count) == (bundle.MAGIC, bundle.VERSION, 16),
            "adjacent bundle header")
    cursor = bundle_at + bundle.HEADER.size
    final_header = -1
    final_length = -1
    for index in range(count):
        entry_at = cursor
        label_length, content_length = bundle.ENTRY_HEADER.unpack_from(exact, entry_at)
        cursor += bundle.ENTRY_HEADER.size + label_length + content_length
        if index == count - 1:
            final_header, final_length = entry_at, content_length
    require(cursor == len(exact) and final_length < 131_072,
            "adjacent final source extent")
    mutated = bytearray(exact)
    struct.pack_into("<I", mutated, final_header + 4, final_length + 1)
    mutated.append(32)
    struct.pack_into("<I", mutated, 16, len(mutated))
    struct.pack_into("<I", mutated, 20, header[6] + 1)
    try:
        compilation.decode(bytes(mutated))
    except compilation.CompilationError as error:
        require(error.status == 252, "adjacent compilation status")
        # The added source space exceeds both source-aggregate and nested-bundle
        # ceilings. Canonical decoder order publishes the bundle extent first.
        require(str(error) == "nested source-bundle byte length exceeds 263312",
                f"adjacent nested-bundle diagnostic: {error}")
    else:
        raise ValueError("adjacent envelope passed production resource checks")
    require(sum(len(entry.content) for entry in decoded.bundle_entries) == SOURCE_BYTES,
            "adjacent exact aggregate base")
    return bytes(mutated)


def check_witness(envelope: Path, witness: Path) -> None:
    decoded_compilation = compilation.decode(envelope.read_bytes())
    decoded_witness = decode_witness(witness.read_bytes())
    require(len(envelope.read_bytes()) == COMPILATION_BYTES, "checked OMGCOMP length")
    require(len(decoded_witness.raw) == WITNESS_BYTES, "checked OMGRSW1 length")
    require(decoded_witness.counts == COUNTS, "greatest witness counts")
    require(decoded_witness.selected == 120, "selected machine")
    require(sum(len(entry.content) for entry in decoded_compilation.bundle_entries)
            == SOURCE_BYTES, "checked source aggregate")


def observe(arguments: list[str]) -> None:
    require(len(arguments) >= 8 and arguments[6] == "--", "observe arguments")
    timeout = float(arguments[0])
    input_name, output_name = arguments[1:3]
    expected = int(arguments[3])
    timing_name, label = arguments[4:6]
    command = arguments[7:]
    started = time.monotonic()
    with (open(input_name, "rb") if input_name != "-" else open("/dev/null", "rb")) as source:
        with (open(output_name, "wb") if output_name != "-" else open("/dev/null", "wb")) as output:
            process = subprocess.Popen(
                command, stdin=source, stdout=output, stderr=subprocess.PIPE,
                start_new_session=True,
            )
            try:
                _, stderr = process.communicate(timeout=timeout)
            except subprocess.TimeoutExpired as error:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate()
                raise ValueError(f"{label} exceeded {timeout:.0f}s") from error
    elapsed = time.monotonic() - started
    with open(timing_name, "a", encoding="ascii") as timings:
        timings.write(f"{elapsed:.6f}\t{label}\n")
    if process.returncode != expected:
        detail = stderr.decode("utf-8", errors="replace")[-1000:]
        raise ValueError(
            f"{label} status {process.returncode}, expected {expected}: {detail}"
        )
    if expected and output_name != "-" and Path(output_name).stat().st_size:
        raise ValueError(f"{label} published bytes on rejection")


def report(timing_path: Path) -> None:
    rows = []
    for line in timing_path.read_text(encoding="ascii").splitlines():
        elapsed, label = line.split("\t", 1)
        rows.append((label, float(elapsed)))
    wanted = {
        "generate", "resolver", "self-source", "native-positive", "self-positive",
        "native-adjacent", "self-adjacent",
    }
    print("greatest OMGLOW3 timings: " + " ".join(
        f"{label}={elapsed:.3f}s" for label, elapsed in rows if label in wanted
    ))


def main(arguments: list[str]) -> int:
    if len(arguments) == 2 and arguments[0] == "build":
        build(Path(arguments[1])); return 0
    if len(arguments) == 3 and arguments[0] == "check-witness":
        check_witness(Path(arguments[1]), Path(arguments[2])); return 0
    if arguments and arguments[0] == "observe":
        observe(arguments[1:]); return 0
    if len(arguments) == 2 and arguments[0] == "report":
        report(Path(arguments[1])); return 0
    raise ValueError(
        "usage: build DIR | check-witness OMGCOMP OMGRSW1 | "
        "observe TIME IN OUT STATUS TIMINGS LABEL -- COMMAND... | report TIMINGS"
    )


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as error:
        print(f"greatest OMGLOW3 fixture: {error}", file=sys.stderr)
        raise SystemExit(2)
