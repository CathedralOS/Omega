#!/usr/bin/env python3
"""Independent fixed-shape OMGCOMP2 -> OMGRSW6 provider reference.

This module deliberately shares no parser, table builder, or conclusion with
the Delta resolver.  It reconstructs the focused Console::exit_process source
relation from the exact OMGCOMP2 bytes and then compares every OMGRSW6 byte.
"""

from __future__ import annotations

import argparse
import json
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path


HERE = Path(__file__).resolve().parent
FIXTURE = HERE / "fixtures" / "omgrsw6-console-provider"
NO_ID = 0xFFFF_FFFF

COMP_HEADER = struct.Struct("<8sHHHH12I")
PACKAGE = struct.Struct("<I32sIII")
SOURCE = struct.Struct("<IIIII")
ALIAS = struct.Struct("<IIII")
BUNDLE_HEADER = struct.Struct("<8sII")
BUNDLE_ENTRY = struct.Struct("<II")
WITNESS_HEADER = struct.Struct("<8sHHHH28I")

UNIT = struct.Struct("<9I")
IMPORT = struct.Struct("<5IBBH6I")
BINDING = struct.Struct("<IIBBH4I")
DECLARATION = struct.Struct("<IBBH5I")
TYPE = struct.Struct("<IBBH4I")
RECORD = struct.Struct("<5IB3x")
FIELD = struct.Struct("<6I")
MACHINE = struct.Struct("<3IBBH6I")
BLOCK = struct.Struct("<3IBBH6I")
TRAIT = struct.Struct("<5IB3x")
REQUIREMENT = struct.Struct("<10I")
PARAMETER = struct.Struct("<6I")
REACH = struct.Struct("<6I")
REALIZATION = struct.Struct("<12I")
CALL = struct.Struct("<10I")

STD_KEY = bytes.fromhex("11" * 32)
APP_KEY = bytes.fromhex("22" * 32)
STRINGS = (b"Main", b"app", b"console", b"main", b"omega_std")
LABELS = (
    "app/main.omg",
    "omega/std/console.omg",
    "omega/std/targets/linux_x64/console_impl.omg",
)

SOURCE_TOKENS = (
    (
        "pub boundary trait Console { machine exit_process ( return_code : i32 ) "
        "reaches Console ; } data ConsoleNativeProvider { }"
    ).split(),
    (
        "linux_x64 machine ConsoleNativeProvider :: exit_process ( return_code : i32 ) "
        "satisfies Console :: exit_process via Binding :: CompilerIntrinsic ;"
    ).split(),
    (
        "use omega_std :: console :: Console ; data Main { console : Console ; } "
        "machine Main :: main ( & mut self ) { self . console . exit_process ( 70 ) ; }"
    ).split(),
)


class ReferenceError(ValueError):
    def __init__(self, message: str, status: int = 251):
        super().__init__(message)
        self.status = status


def require(condition: bool, message: str, status: int = 251) -> None:
    if not condition:
        raise ReferenceError(message, status)


@dataclass(frozen=True)
class Token:
    value: str
    start: int
    end: int


def lex(source: bytes) -> list[Token]:
    tokens: list[Token] = []
    cursor = 0
    while cursor < len(source):
        if source[cursor] in b" \t\r\n":
            cursor += 1
            continue
        if source.startswith(b"//", cursor):
            end = source.find(b"\n", cursor + 2)
            cursor = len(source) if end < 0 else end + 1
            continue
        if source.startswith(b"/*", cursor):
            end = source.find(b"*/", cursor + 2)
            require(end >= 0, "unterminated block comment")
            cursor = end + 2
            continue
        match = re.match(rb"[A-Za-z_][A-Za-z0-9_]*|[0-9]+|::|->|.", source[cursor:])
        require(match is not None, "unlexable source byte")
        raw = match.group(0)
        try:
            value = raw.decode("ascii")
        except UnicodeDecodeError as error:
            raise ReferenceError("non-ASCII source token") from error
        tokens.append(Token(value, cursor, cursor + len(raw)))
        cursor += len(raw)
    return tokens


def validate_sources(sources: tuple[bytes, bytes, bytes]) -> tuple[list[Token], ...]:
    result = tuple(lex(source) for source in sources)
    for source_id, (tokens, expected) in enumerate(zip(result, SOURCE_TOKENS)):
        require([token.value for token in tokens] == expected,
                f"source {source_id} is outside the frozen provider profile")
    return result


def occurrence(tokens: list[Token], value: str, ordinal: int = 0) -> Token:
    matches = [token for token in tokens if token.value == value]
    require(ordinal < len(matches), f"missing token {value!r} occurrence {ordinal}")
    return matches[ordinal]


def span(first: Token, last: Token | None = None) -> tuple[int, int]:
    last = first if last is None else last
    return first.start, last.end - first.start


def encode_bundle(contents: dict[str, bytes]) -> bytes:
    output = bytearray(BUNDLE_HEADER.pack(b"OMG0BNDL", 1, len(LABELS)))
    for label in LABELS:
        raw_label = label.encode("ascii")
        content = contents[label]
        output.extend(BUNDLE_ENTRY.pack(len(raw_label), len(content)))
        output.extend(raw_label)
        output.extend(content)
    return bytes(output)


def decode_bundle(raw: bytes) -> dict[str, bytes]:
    require(len(raw) >= BUNDLE_HEADER.size, "truncated source bundle")
    magic, version, count = BUNDLE_HEADER.unpack_from(raw)
    require((magic, version, count) == (b"OMG0BNDL", 1, 3), "wrong source bundle header")
    cursor = BUNDLE_HEADER.size
    result: dict[str, bytes] = {}
    previous = b""
    for _ in range(count):
        require(cursor + BUNDLE_ENTRY.size <= len(raw), "truncated bundle entry")
        label_length, content_length = BUNDLE_ENTRY.unpack_from(raw, cursor)
        cursor += BUNDLE_ENTRY.size
        end = cursor + label_length + content_length
        require(end <= len(raw), "bundle entry extent")
        label = raw[cursor:cursor + label_length]
        cursor += label_length
        require(label > previous, "noncanonical bundle label order")
        previous = label
        try:
            decoded = label.decode("ascii")
        except UnicodeDecodeError as error:
            raise ReferenceError("non-ASCII bundle label") from error
        require(decoded in LABELS and decoded not in result, "unexpected bundle label")
        result[decoded] = raw[cursor:cursor + content_length]
        cursor += content_length
    require(cursor == len(raw), "trailing source bundle bytes")
    require(tuple(result) == LABELS, "wrong source bundle labels")
    return result


def fixture_contents() -> dict[str, bytes]:
    return {
        "app/main.omg": (FIXTURE / "app-main.omg").read_bytes(),
        "omega/std/console.omg": (FIXTURE / "console.omg").read_bytes(),
        "omega/std/targets/linux_x64/console_impl.omg":
            (FIXTURE / "console-impl-linux-x64.omg").read_bytes(),
    }


def encode_envelope(contents: dict[str, bytes]) -> bytes:
    bundle = encode_bundle(contents)
    string_table = b"".join(struct.pack("<I", len(value)) + value for value in STRINGS)
    fixed = b"".join((
        PACKAGE.pack(0, STD_KEY, 0, 2, 0),
        PACKAGE.pack(1, APP_KEY, 2, 1, 0),
        SOURCE.pack(0, 0, 1, 2, 0),
        SOURCE.pack(1, 0, 2, 2, 0),
        SOURCE.pack(2, 1, 0, 1, 0),
        ALIAS.pack(1, 4, 0, 0),
    ))
    total = COMP_HEADER.size + len(fixed) + len(string_table) + len(bundle)
    return COMP_HEADER.pack(
        b"OMGCOMP\0", 2, 0, 1, 0, total, len(bundle), len(string_table),
        len(STRINGS), 2, 3, 1, 1, 2, 0, 3, 1,
    ) + fixed + string_table + bundle


def decode_envelope(raw: bytes) -> tuple[bytes, bytes, bytes]:
    require(len(raw) <= 267_280, "OMGCOMP2 input ceiling", 252)
    require(len(raw) >= COMP_HEADER.size, "truncated OMGCOMP2 header")
    fields = COMP_HEADER.unpack_from(raw)
    require(fields[:5] == (b"OMGCOMP\0", 2, 0, 1, 0), "wrong OMGCOMP2 identity")
    (total, bundle_length, string_length, string_count, package_count,
     source_count, alias_count, root_package, root_source, root_owner,
     root_machine, configuration) = fields[5:]
    require((total, package_count, source_count, alias_count) ==
            (len(raw), 2, 3, 1), "wrong OMGCOMP2 fixed counts")
    require((root_package, root_source, root_owner, root_machine, configuration) ==
            (1, 2, 0, 3, 1), "wrong OMGCOMP2 root/configuration")
    cursor = COMP_HEADER.size
    require(raw[cursor:cursor + 2 * PACKAGE.size] == b"".join((
        PACKAGE.pack(0, STD_KEY, 0, 2, 0), PACKAGE.pack(1, APP_KEY, 2, 1, 0))),
        "wrong OMGCOMP2 package table")
    cursor += 2 * PACKAGE.size
    require(raw[cursor:cursor + 3 * SOURCE.size] == b"".join((
        SOURCE.pack(0, 0, 1, 2, 0), SOURCE.pack(1, 0, 2, 2, 0),
        SOURCE.pack(2, 1, 0, 1, 0))), "wrong OMGCOMP2 source table")
    cursor += 3 * SOURCE.size
    require(raw[cursor:cursor + ALIAS.size] == ALIAS.pack(1, 4, 0, 0),
            "wrong OMGCOMP2 alias table")
    cursor += ALIAS.size
    expected_strings = b"".join(struct.pack("<I", len(value)) + value for value in STRINGS)
    require((string_count, string_length) == (5, len(expected_strings)),
            "wrong OMGCOMP2 string header")
    require(raw[cursor:cursor + string_length] == expected_strings,
            "wrong OMGCOMP2 string table")
    cursor += string_length
    require(bundle_length == len(raw) - cursor, "wrong OMGCOMP2 bundle extent")
    contents = decode_bundle(raw[cursor:])
    sources = (contents[LABELS[1]], contents[LABELS[2]], contents[LABELS[0]])
    validate_sources(sources)
    require(raw == encode_envelope(contents), "noncanonical OMGCOMP2 bytes")
    return sources


def witness_tables(sources: tuple[bytes, bytes, bytes]) -> list[tuple[str, bytes]]:
    t0, t1, t2 = validate_sources(sources)

    console_decl = occurrence(t0, "Console", 0)
    requirement_name = occurrence(t0, "exit_process")
    requirement_param = occurrence(t0, "return_code")
    reach_target = occurrence(t0, "Console", 1)
    provider_name = occurrence(t0, "ConsoleNativeProvider")

    qualifier = occurrence(t1, "linux_x64")
    realization_owner = occurrence(t1, "ConsoleNativeProvider")
    realization_name = occurrence(t1, "exit_process", 0)
    realization_param = occurrence(t1, "return_code")
    satisfies_console = occurrence(t1, "Console")
    satisfies_requirement = occurrence(t1, "exit_process", 1)

    import_start = occurrence(t2, "omega_std")
    import_console = occurrence(t2, "Console", 0)
    main_data = occurrence(t2, "Main", 0)
    field_name = occurrence(t2, "console", 1)
    field_type = occurrence(t2, "Console", 1)
    main_owner = occurrence(t2, "Main", 1)
    main_name = occurrence(t2, "main")
    body_start = occurrence(t2, "self", 1)
    body_end = t2[-1]
    call_target = occurrence(t2, "exit_process")

    tables: list[tuple[str, bytes]] = []
    tables.append(("units", b"".join((
        UNIT.pack(0, 0, 2, NO_ID, 0, 0, 0, 0, 2),
        UNIT.pack(1, 0, 2, NO_ID, 0, 0, 0, 2, 1),
        UNIT.pack(2, 1, 1, NO_ID, 0, 0, 1, 3, 2),
    ))))
    import_start_at, import_length = span(import_start, import_console)
    local_start, local_length = span(import_console)
    tables.append(("imports", IMPORT.pack(
        0, 2, 0, import_start_at, import_length, 1, 3, 0,
        0, 0, 2, 0, local_start, local_length,
    )))

    binding_rows = (
        (0, 0, 5, 3, *span(reach_target), 0, NO_ID),
        (1, 1, 2, 1, *span(realization_owner), 1, NO_ID),
        (2, 1, 4, 4, *span(satisfies_console, satisfies_requirement), 0, NO_ID),
        (3, 2, 1, 3, *span(field_type), 0, 0),
        (4, 2, 2, 1, *span(main_owner), 3, NO_ID),
        (5, 2, 3, 4, *span(call_target), 0, 0),
    )
    tables.append(("bindings", b"".join(
        BINDING.pack(identifier, source, role, target_kind, 0,
                     start, length, target, import_id)
        for identifier, source, role, target_kind, start, length, target, import_id
        in binding_rows
    )))

    declaration_rows = (
        (0, 4, 1, 0, 0, *span(console_decl), 0),
        (1, 1, 0, 0, 1, *span(provider_name), 0),
        (2, 5, 0, 1, 0, *span(realization_name), 0),
        (3, 1, 0, 2, 0, *span(main_data), 1),
        (4, 2, 0, 2, 1, *span(main_name), 0),
    )
    tables.append(("declarations", b"".join(
        DECLARATION.pack(identifier, kind, visibility, 0, source, ordinal,
                         start, length, table_id)
        for identifier, kind, visibility, source, ordinal, start, length, table_id
        in declaration_rows
    )))
    tables.append(("types", b"".join((
        TYPE.pack(0, 4, 0, 0, 0, 0, 0, 0),
        TYPE.pack(1, 4, 0, 0, 1, 0, 0, 0),
        TYPE.pack(2, 8, 0, 0, 0, 0, 0, 0),
        TYPE.pack(3, 9, 0, 0, 0, 0, 0, 0),
    ))))
    tables.append(("records", b"".join((
        RECORD.pack(0, 1, 0, 0, 0, 0),
        RECORD.pack(1, 3, 1, 0, 1, 0),
    ))))
    tables.append(("fields", FIELD.pack(0, 1, 0, 3, *span(field_name))))
    tables.extend((("sums", b""), ("cases", b""), ("payloads", b"")))
    tables.append(("machines", MACHINE.pack(
        0, 4, 1, 2, 0, 0, NO_ID, 0, 0, 0, 1, 0,
    )))
    tables.append(("machine_parameters", b""))
    tables.append(("blocks", BLOCK.pack(
        0, 0, 0, 2, 0, 0, body_start.start, body_end.start,
        NO_ID, 0, 0, 0,
    )))
    tables.append(("block_parameters", b""))
    tables.append(("traits", TRAIT.pack(0, 0, 3, 0, 1, 1)))
    tables.append(("requirements", REQUIREMENT.pack(
        0, 0, 0, NO_ID, 0, 1, 0, 1, *span(requirement_name),
    )))
    tables.append(("requirement_parameters", PARAMETER.pack(
        0, 0, 0, 2, *span(requirement_param),
    )))
    tables.append(("reaches", REACH.pack(
        0, 0, 0, 0, *span(reach_target),
    )))
    tables.append(("realizations", REALIZATION.pack(
        0, 2, 0, 1, 0, 1, NO_ID, 0, 1,
        *span(realization_name), qualifier.start,
    )))
    tables.append(("realization_parameters", PARAMETER.pack(
        0, 0, 0, 2, *span(realization_param),
    )))
    tables.append(("requirement_calls", CALL.pack(
        0, 2, 0, 0, 0, 0, *span(call_target), 1, 0,
    )))
    return tables


HEADER_COUNTS = (
    3, 1, 6, 5, 4, 2, 1, 1, 0, 1, 0, 0, 0, 0,
    1, 1, 1, 1, 1, 1, 1,
)


def encode_witness(envelope: bytes) -> bytes:
    sources = decode_envelope(envelope)
    tables = witness_tables(sources)
    payload = b"".join(raw for _, raw in tables)
    total = WITNESS_HEADER.size + len(payload)
    require(total == 1_064, f"internal OMGRSW6 length {total}, expected 1064")
    header = WITNESS_HEADER.pack(
        b"OMGRSW6\0", 6, 0, 0, 128, total, *HEADER_COUNTS,
        0, 1, 1, 0, 0, 0,
    )
    return header + payload


def decode_witness(envelope: bytes, raw: bytes) -> dict[str, object]:
    sources = decode_envelope(envelope)
    require(len(raw) <= 524_288, "OMGRSW6 carrier ceiling", 252)
    require(len(raw) >= WITNESS_HEADER.size, "truncated OMGRSW6 header")
    fields = WITNESS_HEADER.unpack_from(raw)
    require(fields[:5] == (b"OMGRSW6\0", 6, 0, 0, 128), "wrong OMGRSW6 identity")
    require(fields[5] == len(raw), "OMGRSW6 exact length")
    require(fields[6:27] == HEADER_COUNTS, "wrong OMGRSW6 fixed counts")
    require(fields[27:] == (0, 1, 1, 0, 0, 0), "wrong OMGRSW6 root/target/configuration")
    expected_tables = witness_tables(sources)
    cursor = WITNESS_HEADER.size
    for name, expected in expected_tables:
        observed = raw[cursor:cursor + len(expected)]
        require(len(observed) == len(expected), f"truncated OMGRSW6 {name} table")
        require(observed == expected, f"OMGRSW6 {name} table differs")
        cursor += len(expected)
    require(cursor == len(raw), "trailing OMGRSW6 bytes")
    require(raw == encode_witness(envelope), "noncanonical OMGRSW6 bytes")
    for forbidden in (b"ProviderPlan", b"select_provider", b"catalog", b"CKIR", b"ELF"):
        require(forbidden not in raw, "OMGRSW6 implies a forbidden downstream relation")
    return {
        "schema": "OMGRSW6",
        "bytes": len(raw),
        "selected_machine": 0,
        "requirement": "Console::exit_process(i32)->Unit",
        "reach": "Console",
        "realization": "console::ConsoleNativeProvider::exit_process(i32)->Unit",
        "candidate": {"kind": "CompilerIntrinsic", "payload_bytes": 0, "target": 1},
        "call_target": {"kind": "requirement", "id": 0},
        "selection": None,
    }


def replace_u16(raw: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(raw)
    struct.pack_into("<H", changed, offset, value)
    return bytes(changed)


def replace_u32(raw: bytes, offset: int, value: int) -> bytes:
    changed = bytearray(raw)
    struct.pack_into("<I", changed, offset, value)
    return bytes(changed)


def expect_reject(name: str, status: int, action) -> None:
    try:
        action()
    except ReferenceError as error:
        require(error.status == status, f"{name}: status {error.status}, expected {status}")
    else:
        raise ReferenceError(f"{name}: unexpectedly accepted")


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    contents = fixture_contents()
    envelope = encode_envelope(contents)
    witness = encode_witness(envelope)
    decode_witness(envelope, witness)
    (output / "canonical.omgc").write_bytes(envelope)
    (output / "canonical.omgrsw6").write_bytes(witness)
    (output / "inspection.json").write_text(
        json.dumps(decode_witness(envelope, witness), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    semantic_cases: list[tuple[str, int, bytes]] = []
    changed = dict(contents)
    changed[LABELS[2]] = changed[LABELS[2]].replace(
        b"CompilerIntrinsic", b"CompilerIntrinsix")
    semantic_cases.append(("wrong-candidate", 251, encode_envelope(changed)))
    changed = dict(contents)
    changed[LABELS[0]] = changed[LABELS[0]].replace(b"exit_process(70)", b"exit_procesx(70)")
    semantic_cases.append(("wrong-call-target", 251, encode_envelope(changed)))
    changed = dict(contents)
    changed[LABELS[2]] += (
        b"\nlinux_x64 machine ConsoleNativeProvider::provider_defaults() { "
        b"select_provider(); }\n"
    )
    semantic_cases.append(("forbidden-selection", 251, encode_envelope(changed)))
    changed = dict(contents)
    changed[LABELS[1]] = changed[LABELS[1]].replace(
        b"ConsoleNativeProvider", b"P" * 65)
    semantic_cases.append(("resource-identifier-adjacent", 252, encode_envelope(changed)))
    semantic_cases.extend((
        ("wrong-target", 251, replace_u16(envelope, 12, 2)),
        ("truncated-eof", 251, envelope[:-1]),
        ("trailing-eof", 251, envelope + b"x"),
        ("resource-input-adjacent", 252, bytes(267_281)),
    ))
    with (output / "resolver-cases.tsv").open("w", encoding="utf-8") as rows:
        for name, status, raw in semantic_cases:
            path = output / f"{name}.omgc"
            path.write_bytes(raw)
            rows.write(f"{name}\t{status}\t{path}\n")

    # Representative mutations cover identity, every new semantic table class,
    # exact framing, and the call-versus-candidate separation.  Complete byte
    # equality makes every other row/field mutation reject by construction.
    tables = dict(witness_tables(decode_envelope(envelope)))
    offsets: dict[str, int] = {}
    cursor = WITNESS_HEADER.size
    for name, raw in witness_tables(decode_envelope(envelope)):
        offsets[name] = cursor
        cursor += len(raw)
    witness_mutations = {
        "magic": b"X" + witness[1:],
        "major": replace_u16(witness, 8, 5),
        "count": replace_u32(witness, 80, 2),
        "binding-target": replace_u32(witness, offsets["bindings"] + 5 * BINDING.size + 20, 1),
        "trait-flags": bytes(bytearray(witness[:offsets["traits"] + 20]) +
                             bytes([0]) + witness[offsets["traits"] + 21:]),
        "requirement-result": replace_u32(witness, offsets["requirements"] + 12, 2),
        "reach-target": replace_u32(witness, offsets["reaches"] + 12, 1),
        "candidate-kind": replace_u32(witness, offsets["realizations"] + 20, 2),
        "call-requirement": replace_u32(witness, offsets["requirement_calls"] + 20, 1),
        "truncated": witness[:-1],
        "trailing": witness + b"x",
    }
    require(tables["realizations"], "missing realization table")
    for name, mutated in witness_mutations.items():
        expect_reject(name, 251, lambda raw=mutated: decode_witness(envelope, raw))
    expect_reject(
        "witness-resource-adjacent", 252,
        lambda: decode_witness(envelope, bytes(524_289)),
    )
    print("OMGRSW6 independent reference: exact 1064-byte witness and mutations PASS")


def check(envelope: Path, witness: Path) -> None:
    view = decode_witness(envelope.read_bytes(), witness.read_bytes())
    print(json.dumps(view, sort_keys=True))


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    build_parser = subparsers.add_parser("build")
    build_parser.add_argument("output", type=Path)
    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("envelope", type=Path)
    check_parser.add_argument("witness", type=Path)
    arguments = parser.parse_args()
    if arguments.command == "build":
        build(arguments.output)
    else:
        check(arguments.envelope, arguments.witness)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ReferenceError) as error:
        status = error.status if isinstance(error, ReferenceError) else 251
        print(f"OMGRSW6 independent reference: {error}", file=sys.stderr)
        raise SystemExit(status)
