#!/usr/bin/env python3
"""Independent exact-source OMGCOMP3 -> OMGRSW9 provider-plan reference.

The reference shares no source parser, semantic tables, or conclusion with the
Delta resolver.  It independently decodes the four-source envelope, parses the
frozen Console product slice, reconstructs every V9 row, and compares every
published byte.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path


HERE = Path(__file__).resolve().parent
FIXTURE = HERE / "fixtures" / "omgcomp3-console-provider-plan"
NO_ID = 0xFFFF_FFFF
WITNESS_BYTES = 2_304

COMP_HEADER = struct.Struct("<8sHHHH12I")
PACKAGE = struct.Struct("<I32sIII")
SOURCE = struct.Struct("<IIIII")
ALIAS = struct.Struct("<IIII")
BUNDLE_HEADER = struct.Struct("<8sII")
BUNDLE_ENTRY = struct.Struct("<II")
WITNESS_HEADER = struct.Struct("<8sHHHH32I")

UNIT = struct.Struct("<7I")
TYPE = struct.Struct("<IBBH4I")
TRAIT = struct.Struct("<8I")
REQUIREMENT = struct.Struct("<12I")
PARAMETER = struct.Struct("<6I")
REACH = struct.Struct("<6I")
PROVIDER = struct.Struct("<6I")
HELPER = struct.Struct("<12I")
ADAPTER = struct.Struct("<13I")
CANDIDATE = struct.Struct("<14I")
BUILD_MACHINE = struct.Struct("<10I")
SELECTION = struct.Struct("<9I")
PLAN = struct.Struct("<9I")
PLAN_ROW = struct.Struct("<6I")
REQUIREMENT_CALL = struct.Struct("<11I")
ORDINARY_CALL = struct.Struct("<9I")

STD_KEY = bytes.fromhex("11" * 32)
APP_KEY = bytes.fromhex("22" * 32)
STRINGS = (b"Main", b"app", b"console", b"main", b"omega_std")
LABELS = (
    "app/build.omg",
    "app/main.omg",
    "omega/std/console.omg",
    "omega/std/targets/linux_x64/console_impl.omg",
)
SOURCE_LABELS = (LABELS[2], LABELS[3], LABELS[0], LABELS[1])
SOURCE_PROFILE_SHA256 = (
    "967ec23e48c0d10c0f8896e6c1e16ea4d8507ef14a0c200d7cba72518f201b1d",
    "a6cc1f99f25a588179460c6d16c84e14f6a007de4f9da06b93a2d6e5679581ec",
    "150cc1b2480fa2736878365453582a5517a2571eb0cb85b930400ecc89ffab7b",
    "baf34421ab672cb059f31fddd4d2a8ac22929698035261583dca975d5859f63e",
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
        match = re.match(
            rb"[A-Za-z_][A-Za-z0-9_]*|[0-9]+|::|->|\.\.=|.", source[cursor:]
        )
        require(match is not None, "unlexable source byte")
        raw = match.group(0)
        try:
            value = raw.decode("ascii")
        except UnicodeDecodeError as error:
            raise ReferenceError("non-ASCII source token") from error
        tokens.append(Token(value, cursor, cursor + len(raw)))
        cursor += len(raw)
    return tokens


def occurrence(tokens: list[Token], value: str, ordinal: int = 0) -> Token:
    matches = [token for token in tokens if token.value == value]
    require(ordinal < len(matches), f"missing token {value!r} occurrence {ordinal}")
    return matches[ordinal]


def sequence(tokens: list[Token], values: tuple[str, ...], ordinal: int = 0) -> tuple[Token, Token]:
    found = []
    for index in range(len(tokens) - len(values) + 1):
        if tuple(token.value for token in tokens[index:index + len(values)]) == values:
            found.append((tokens[index], tokens[index + len(values) - 1]))
    require(ordinal < len(found), f"missing token sequence {values!r} occurrence {ordinal}")
    return found[ordinal]


def span(first: Token, last: Token | None = None) -> tuple[int, int]:
    last = first if last is None else last
    return first.start, last.end - first.start


def top_level_chunks(tokens: list[Token]) -> list[list[Token]]:
    starts: list[int] = []
    depth = 0
    for index, token in enumerate(tokens):
        is_start = token.value in ("pub", "use", "machine") or (
            token.value == "data" and (index == 0 or tokens[index - 1].value != "pub")
        )
        if depth == 0 and is_start:
            starts.append(index)
        if token.value == "{":
            depth += 1
        elif token.value == "}":
            depth -= 1
            require(depth >= 0, "unbalanced top-level declaration")
    require(depth == 0 and starts, "unbalanced or empty source")
    starts.append(len(tokens))
    return [tokens[start:end] for start, end in zip(starts, starts[1:])]


def source_profile_digest(tokens: list[Token], reorder_independent: bool) -> str:
    chunks = top_level_chunks(tokens) if reorder_independent else [tokens]
    encoded = [b"\x1f".join(token.value.encode("ascii") for token in chunk)
               for chunk in chunks]
    if reorder_independent:
        encoded.sort()
    return hashlib.sha256(b"\x1e".join(encoded)).hexdigest()


def fixture_contents() -> dict[str, bytes]:
    return {
        LABELS[0]: (FIXTURE / "build.omg").read_bytes(),
        LABELS[1]: (FIXTURE / "app-main.omg").read_bytes(),
        LABELS[2]: (FIXTURE / "console.omg").read_bytes(),
        LABELS[3]: (FIXTURE / "console-impl-linux-x64.omg").read_bytes(),
    }


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
    require(BUNDLE_HEADER.unpack_from(raw) == (b"OMG0BNDL", 1, 4), "wrong bundle header")
    cursor = BUNDLE_HEADER.size
    result: dict[str, bytes] = {}
    previous = b""
    for _ in range(4):
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
    require(tuple(result) == LABELS, "wrong bundle labels")
    return result


def encode_envelope(contents: dict[str, bytes], build_source: int = 2) -> bytes:
    bundle = encode_bundle(contents)
    string_table = b"".join(struct.pack("<I", len(value)) + value for value in STRINGS)
    source_flags = [0, 0, 0, 0]
    require(0 <= build_source < 4, "invalid build source")
    source_flags[build_source] = 1
    fixed = b"".join((
        PACKAGE.pack(0, STD_KEY, 0, 2, 0),
        PACKAGE.pack(1, APP_KEY, 2, 2, 0),
        SOURCE.pack(0, 0, 2, 2, source_flags[0]),
        SOURCE.pack(1, 0, 3, 2, source_flags[1]),
        SOURCE.pack(2, 1, 0, 1, source_flags[2]),
        SOURCE.pack(3, 1, 1, 1, source_flags[3]),
        ALIAS.pack(1, 4, 0, 0),
    ))
    total = COMP_HEADER.size + len(fixed) + len(string_table) + len(bundle)
    return COMP_HEADER.pack(
        b"OMGCOMP\0", 3, 0, 1, 0, total, len(bundle), len(string_table),
        len(STRINGS), 2, 4, 1, 1, 3, 0, 3, 1,
    ) + fixed + string_table + bundle


def decode_envelope(raw: bytes) -> tuple[tuple[bytes, ...], int, int]:
    require(len(raw) <= 267_280, "OMGCOMP3 input ceiling", 252)
    require(len(raw) >= COMP_HEADER.size, "truncated OMGCOMP3 header")
    fields = COMP_HEADER.unpack_from(raw)
    require(fields[:5] == (b"OMGCOMP\0", 3, 0, 1, 0), "wrong OMGCOMP3 identity")
    (total, bundle_length, string_length, string_count, package_count,
     source_count, alias_count, root_package, root_source, root_owner,
     root_machine, configuration) = fields[5:]
    require((total, package_count, source_count, alias_count) ==
            (len(raw), 2, 4, 1), "wrong OMGCOMP3 fixed counts")
    require((root_package, root_source, root_owner, root_machine, configuration) ==
            (1, 3, 0, 3, 1), "wrong OMGCOMP3 root/configuration")
    cursor = COMP_HEADER.size
    expected_packages = b"".join((
        PACKAGE.pack(0, STD_KEY, 0, 2, 0), PACKAGE.pack(1, APP_KEY, 2, 2, 0)
    ))
    require(raw[cursor:cursor + len(expected_packages)] == expected_packages,
            "wrong OMGCOMP3 package table")
    cursor += len(expected_packages)
    rows = [SOURCE.unpack_from(raw, cursor + index * SOURCE.size) for index in range(4)]
    require(tuple(row[:4] for row in rows) == (
        (0, 0, 2, 2), (1, 0, 3, 2), (2, 1, 0, 1), (3, 1, 1, 1),
    ), "wrong OMGCOMP3 source table")
    build_rows = [index for index, row in enumerate(rows) if row[4] == 1]
    require(len(build_rows) == 1 and all(row[4] in (0, 1) for row in rows),
            "wrong OMGCOMP3 build-source role")
    require(rows[build_rows[0]][1] == root_package, "build source outside root package")
    cursor += 4 * SOURCE.size
    require(raw[cursor:cursor + ALIAS.size] == ALIAS.pack(1, 4, 0, 0),
            "wrong OMGCOMP3 alias table")
    cursor += ALIAS.size
    expected_strings = b"".join(struct.pack("<I", len(value)) + value for value in STRINGS)
    require((string_count, string_length) == (5, len(expected_strings)),
            "wrong OMGCOMP3 string header")
    require(raw[cursor:cursor + string_length] == expected_strings,
            "wrong OMGCOMP3 string table")
    cursor += string_length
    require(bundle_length == len(raw) - cursor, "wrong OMGCOMP3 bundle extent")
    contents = decode_bundle(raw[cursor:])
    require(raw == encode_envelope(contents, build_rows[0]), "noncanonical OMGCOMP3 bytes")
    # Resolve source content through the custodied source-row bundle IDs.  The
    # readable labels are validated transport data, never source-role authority.
    bundle_contents = tuple(contents.values())
    sources = tuple(bundle_contents[row[2]] for row in rows)
    return sources, build_rows[0], root_source


def validate_profile(sources: tuple[bytes, ...]) -> tuple[list[Token], ...]:
    tokens = tuple(lex(source) for source in sources)
    profiles = tuple(source_profile_digest(source_tokens, source_id < 2)
                     for source_id, source_tokens in enumerate(tokens))
    require(profiles == SOURCE_PROFILE_SHA256,
            "source semantics outside frozen OMGRSW9 profile")
    # Independent grammar landmarks guard against treating the source digest as
    # semantic authority.
    require(sequence(tokens[0], ("pub", "boundary", "trait", "Console")), "trait")
    require(len([token for token in tokens[0] if token.value == "reaches"]) == 9,
            "wrong portable reach count")
    require(len([token for token in tokens[1] if token.value == "CompilerIntrinsic"]) == 4,
            "wrong intrinsic leaf count")
    require(sequence(tokens[2], ("builder", ".", "select_provider", "<", "Console", ",",
                                  "ConsoleNativeProvider", ">", "(", ")")), "selection")
    require(len([token for token in tokens[3] if token.value in
                 ("read_byte", "write_byte", "exit_process")]) == 4,
            "wrong app requirement-call count")
    return tokens


def body_span(tokens: list[Token], machine_name: str, ordinal: int = 0) -> tuple[int, int]:
    name = occurrence(tokens, machine_name, ordinal)
    start_index = tokens.index(name)
    opening = next(token for token in tokens[start_index:] if token.value == "{")
    depth = 0
    for token in tokens[tokens.index(opening):]:
        if token.value == "{":
            depth += 1
        elif token.value == "}":
            depth -= 1
            if depth == 0:
                return opening.start, token.end - opening.start
    raise ReferenceError(f"unterminated body for {machine_name}")


def body_after(tokens: list[Token], values: tuple[str, ...], ordinal: int = 0) -> tuple[int, int]:
    first, last = sequence(tokens, values, ordinal)
    start_index = tokens.index(last) + 1
    opening = next(token for token in tokens[start_index:] if token.value == "{")
    depth = 0
    for token in tokens[tokens.index(opening):]:
        if token.value == "{":
            depth += 1
        elif token.value == "}":
            depth -= 1
            if depth == 0:
                return opening.start, token.end - opening.start
    raise ReferenceError(f"unterminated body after {first.value}")


def invocation(tokens: list[Token], values: tuple[str, ...], ordinal: int = 0) -> tuple[int, int]:
    _, callee = sequence(tokens, values, ordinal)
    cursor = tokens.index(callee) + 1
    while cursor < len(tokens) and tokens[cursor].value != "(":
        cursor += 1
    require(cursor < len(tokens), f"missing invocation after {values!r}")
    depth = 0
    for token in tokens[cursor:]:
        if token.value == "(":
            depth += 1
        elif token.value == ")":
            depth -= 1
            if depth == 0:
                return callee.start, token.end - callee.start
    raise ReferenceError(f"unterminated invocation after {values!r}")


def named_machine(tokens: list[Token], owner: str, name: str) -> Token:
    _, result = sequence(tokens, ("machine", owner, "::", name))
    return result


def witness_tables(sources: tuple[bytes, ...]) -> list[tuple[str, bytes]]:
    portable, target, build, app = validate_profile(sources)

    requirement_names = ("write_line", "write", "read_line", "read_byte",
                         "write_byte", "exit_process")
    requirement_tokens = [sequence(portable, ("machine", name))[1]
                          for name in requirement_names]
    requirement_params = (
        (0, "text", 5), (1, "text", 5), (2, "out_line", 6),
        (4, "byte", 1), (5, "return_code", 1),
    )
    param_tokens = [sequence(portable, ("machine", requirement_names[req]))[1]
                    for req, _, _ in requirement_params]
    # Advance from each requirement name to its parameter identifier.
    for index, (req, name, _) in enumerate(requirement_params):
        start = portable.index(param_tokens[index]) + 1
        param_tokens[index] = next(token for token in portable[start:] if token.value == name)
    reach_pairs = [sequence(portable, ("reaches", "Console"), ordinal)
                   for ordinal in range(6)]

    trait_name = sequence(portable, ("pub", "boundary", "trait", "Console"))[1]
    provider_name = sequence(portable, ("data", "ConsoleNativeProvider"))[1]
    byte_read_name = sequence(portable, ("pub", "data", "ByteRead"))[1]
    helper_name = sequence(portable, ("machine", "console_write_bytes"))[1]
    helper_body = body_after(portable, ("machine", "console_write_bytes"))

    adapter_requirements = {"write": 1, "write_line": 0}
    adapter_tokens = {
        name: named_machine(portable, "ConsoleNativeProvider", name)
        for name in adapter_requirements
    }
    adapter_order = sorted(adapter_requirements, key=lambda name: adapter_tokens[name].start)
    adapter_specs = tuple(
        (name, adapter_requirements[name], identifier)
        for identifier, name in enumerate(adapter_order)
    )
    adapter_ids = {name: identifier for identifier, name in enumerate(adapter_order)}
    adapter_names = [adapter_tokens[name] for name in adapter_order]
    adapter_bodies = [body_after(portable, ("machine", "ConsoleNativeProvider", "::", name))
                      for name, _, _ in adapter_specs]
    adapter_calls = [invocation(portable, ("console_write_bytes",), ordinal + 1)
                     for ordinal in range(2)]

    target_machine_order = ("read_line", "read_byte", "write_byte", "exit_process")
    target_token_by_name = {
        name: named_machine(target, "ConsoleNativeProvider", name)
        for name in target_machine_order
    }
    target_names = [target_token_by_name[name] for name in target_machine_order]
    target_source_order = sorted(target_machine_order,
                                 key=lambda name: target_token_by_name[name].start)
    target_implementation = {
        name: identifier for identifier, name in enumerate(target_source_order)
    }
    target_params: list[Token | None] = []
    for machine, param in (("read_line", "out_line"), ("read_byte", None),
                           ("write_byte", "byte"), ("exit_process", "return_code")):
        name = named_machine(target, "ConsoleNativeProvider", machine)
        if param is None:
            target_params.append(None)
        else:
            target_params.append(next(token for token in target[target.index(name) + 1:]
                                      if token.value == param))

    build_name = sequence(build, ("machine", "build"))[1]
    application_span = invocation(build, ("builder", ".", "application"))
    selection_span = invocation(build, ("builder", ".", "select_provider"))

    helper_requirement_calls = [invocation(portable, ("console", ".", "write_byte"), ordinal)
                                for ordinal in range(2)]
    app_call_specs = (
        (("self", ".", "console", ".", "read_byte"), 3, 0, 4),
        (("self", ".", "console", ".", "write_byte"), 4, 1, 0),
        (("self", ".", "console", ".", "exit_process"), 5, 1, 0),
        (("self", ".", "console", ".", "exit_process"), 5, 1, 0),
    )
    app_requirement_calls = [
        (invocation(app, values, sum(1 for prior, *_ in app_call_specs[:index]
                                     if prior == values)), req, argc, result)
        for index, (values, req, argc, result) in enumerate(app_call_specs)
    ]

    tables: list[tuple[str, bytes]] = []
    tables.append(("units", b"".join((
        UNIT.pack(0, 0, 0, 2, 0, 0, len(sources[0])),
        UNIT.pack(1, 1, 0, 2, 0, 0, len(sources[1])),
        UNIT.pack(2, 2, 1, 1, 1, 0, len(sources[2])),
        UNIT.pack(3, 3, 1, 1, 2, 0, len(sources[3])),
    ))))
    type_rows = (
        (0, 0, NO_ID, NO_ID, 0, 0),
        (1, 1, NO_ID, NO_ID, 0x8000_0000, 0x7fff_ffff),
        (2, 2, NO_ID, NO_ID, 0, 255),
        (3, 3, NO_ID, NO_ID, 0, 1),
        (4, 4, 0, 0, 0, 0),
        (5, 5, NO_ID, NO_ID, 2, 0),
        (6, 6, NO_ID, NO_ID, 2, 0),
        (7, 7, 0, 0, 0, 0),
    )
    tables.append(("types", b"".join(TYPE.pack(identifier, kind, 0, 0, decl_source,
                                                 decl_id, payload0, payload1)
                                       for identifier, kind, decl_source, decl_id,
                                       payload0, payload1 in type_rows)))
    tables.append(("traits", TRAIT.pack(0, 0, 0, *span(trait_name), 0, 6, 3)))

    result_types = (0, 0, 0, 4, 0, 0)
    param_starts = (0, 1, 2, 3, 3, 4)
    param_counts = (1, 1, 1, 0, 1, 1)
    tables.append(("requirements", b"".join(
        REQUIREMENT.pack(identifier, 0, identifier, 0, *span(requirement_tokens[identifier]),
                         param_starts[identifier], param_counts[identifier],
                         result_types[identifier], identifier, 1, 0)
        for identifier in range(6)
    )))
    tables.append(("requirement_parameters", b"".join(
        PARAMETER.pack(identifier, req, 0, type_id, *span(param_tokens[identifier]))
        for identifier, (req, _, type_id) in enumerate(requirement_params)
    )))
    tables.append(("reaches", b"".join(
        REACH.pack(identifier, identifier, 0, 0, *span(reach_pairs[identifier][1]))
        for identifier in range(6)
    )))
    tables.append(("providers", PROVIDER.pack(0, 0, 0, *span(provider_name), 1)))
    tables.append(("helpers", HELPER.pack(
        0, 0, *span(helper_name), 7, 5, 3, 1, 1, 0, *helper_body
    )))
    tables.append(("adapters", b"".join(
        ADAPTER.pack(identifier, 0, 0, requirement, *span(adapter_names[identifier]),
                     7, 5, 0, call_id, *adapter_bodies[identifier], 1)
        for identifier, (_, requirement, call_id) in enumerate(adapter_specs)
    )))

    candidate_names = (
        adapter_tokens["write_line"], adapter_tokens["write"], *target_names
    )
    candidate_rows = (
        (0, 1, 0, 0, 0, adapter_ids["write_line"], 0, 2, 0, 1),
        (1, 1, 0, 0, 1, adapter_ids["write"], 2, 2, 0, 1),
        (2, 2, 1, 0, 2, target_implementation["read_line"], 4, 1, 0, 2),
        (3, 2, 1, 0, 3, target_implementation["read_byte"], 5, 0, 4, 2),
        (4, 2, 1, 0, 4, target_implementation["write_byte"], 5, 1, 0, 2),
        (5, 2, 1, 0, 5, target_implementation["exit_process"], 6, 1, 0, 2),
    )
    tables.append(("candidates", b"".join(
        CANDIDATE.pack(identifier, kind, source, provider, requirement, implementation,
                       *span(candidate_names[identifier]), parameter_start, parameter_count,
                       result_type, 0, 0 if kind == 1 else 1, binding)
        for (identifier, kind, source, provider, requirement, implementation,
             parameter_start, parameter_count, result_type, binding) in candidate_rows
    )))

    checked_parameter_tokens = []
    for name in ("write_line", "write"):
        machine_name = named_machine(portable, "ConsoleNativeProvider", name)
        suffix = portable[portable.index(machine_name) + 1:]
        checked_parameter_tokens.extend((
            next(token for token in suffix if token.value == "console"),
            next(token for token in suffix if token.value == "text"),
        ))
    candidate_parameter_specs = (
        (0, 0, 7, checked_parameter_tokens[0]),
        (0, 1, 5, checked_parameter_tokens[1]),
        (1, 0, 7, checked_parameter_tokens[2]),
        (1, 1, 5, checked_parameter_tokens[3]),
        (2, 0, 6, target_params[0]),
        (4, 0, 1, target_params[2]),
        (5, 0, 1, target_params[3]),
    )
    tables.append(("candidate_parameters", b"".join(
        PARAMETER.pack(identifier, candidate, ordinal, type_id, *span(token))
        for identifier, (candidate, ordinal, type_id, token)
        in enumerate(candidate_parameter_specs) if token is not None
    )))
    tables.append(("build_machines", BUILD_MACHINE.pack(
        0, 2, *span(build_name), *application_span, *selection_span, 1, 0
    )))
    tables.append(("selections", SELECTION.pack(
        0, 2, 0, 0, 0, *selection_span, 1, 0
    )))
    # The plan originates with the selected provider declaration's package;
    # BuildOverride is separate selection provenance owned by the root package.
    tables.append(("plans", PLAN.pack(0, 0, 0, 1, 0, 0, 6, 0, 1)))
    bindings = (1, 1, 2, 2, 2, 2)
    tables.append(("plan_rows", b"".join(
        PLAN_ROW.pack(identifier, 0, identifier, identifier, identifier,
                      bindings[identifier]) for identifier in range(6)
    )))
    requirement_call_rows = [
        (0, 0, 1, 0, 4, helper_requirement_calls[0], 1, 0),
        (1, 0, 1, 0, 4, helper_requirement_calls[1], 1, 0),
    ]
    requirement_call_rows.extend(
        (identifier + 2, 3, 2, 0, req, call_span, argc, result)
        for identifier, (call_span, req, argc, result) in enumerate(app_requirement_calls)
    )
    tables.append(("requirement_calls", b"".join(
        REQUIREMENT_CALL.pack(identifier, source, caller_kind, caller_id, requirement,
                              *call_span, 7, argument_count, result_type, 0)
        for (identifier, source, caller_kind, caller_id, requirement, call_span,
             argument_count, result_type) in requirement_call_rows
    )))
    tables.append(("ordinary_calls", b"".join(
        ORDINARY_CALL.pack(identifier, 0, 1, identifier, 0, *adapter_calls[identifier], 3, 0)
        for identifier in range(2)
    )))
    return tables


HEADER_COUNTS = (4, 8, 1, 6, 5, 6, 1, 1, 2, 6, 7, 1, 1, 1, 6, 6, 2)


def encode_witness(envelope: bytes) -> bytes:
    sources, build_source, root_source = decode_envelope(envelope)
    require(build_source == 2, "wrong authoritative build source")
    require(root_source == 3, "wrong selected root source")
    tables = witness_tables(sources)
    payload = b"".join(raw for _, raw in tables)
    header = WITNESS_HEADER.pack(
        b"OMGRSW9\0", 9, 0, 0, 144, WITNESS_BYTES, len(envelope),
        *HEADER_COUNTS, build_source, root_source, 1, 1, 0, 0, 0,
        0, 0, 0, 0, 0, 0,
    )
    require(len(header) + len(payload) == WITNESS_BYTES,
            f"internal OMGRSW9 length {len(header) + len(payload)}")
    return header + payload


def decode_witness(envelope: bytes, raw: bytes) -> dict[str, object]:
    sources, build_source, root_source = decode_envelope(envelope)
    require(len(raw) <= 524_288, "OMGRSW9 carrier ceiling", 252)
    require(len(raw) >= WITNESS_HEADER.size, "truncated OMGRSW9 header")
    fields = WITNESS_HEADER.unpack_from(raw)
    require(fields[:5] == (b"OMGRSW9\0", 9, 0, 0, 144), "wrong OMGRSW9 identity")
    require(fields[5:7] == (WITNESS_BYTES, len(envelope)), "wrong OMGRSW9 extents")
    require(fields[7:24] == HEADER_COUNTS, "wrong OMGRSW9 counts")
    require(fields[24:31] == (build_source, root_source, 1, 1, 0, 0, 0),
            "wrong OMGRSW9 selected identities")
    require(fields[31:] == (0, 0, 0, 0, 0, 0), "nonzero OMGRSW9 reserved words")
    require(len(raw) == WITNESS_BYTES, "OMGRSW9 exact length")
    expected_tables = witness_tables(sources)
    cursor = WITNESS_HEADER.size
    for name, expected in expected_tables:
        observed = raw[cursor:cursor + len(expected)]
        require(len(observed) == len(expected), f"truncated OMGRSW9 {name} table")
        require(observed == expected, f"OMGRSW9 {name} table differs")
        cursor += len(expected)
    require(cursor == len(raw), "trailing OMGRSW9 bytes")
    require(raw == encode_witness(envelope), "noncanonical OMGRSW9 bytes")
    return {
        "schema": "OMGRSW9",
        "bytes": len(raw),
        "selected_plan": 0,
        "selected_trait": 0,
        "selected_provider": 0,
        "selection": "BuildOverride",
        "requirements": 6,
        "plan_rows": 6,
        "checked_adapters": 2,
        "intrinsic_leaves": 4,
        "requirement_calls": 6,
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


def semantic_case(contents: dict[str, bytes], label: str, old: bytes, new: bytes,
                  *, count: int = 1, build_source: int = 2) -> bytes:
    changed = dict(contents)
    require(changed[label].count(old) >= count, f"mutation spelling absent: {old!r}")
    changed[label] = changed[label].replace(old, new, count)
    return encode_envelope(changed, build_source)


def positive_variants(contents: dict[str, bytes]) -> list[tuple[str, bytes]]:
    portable_label = LABELS[2]
    target_label = LABELS[3]

    comment = dict(contents)
    marker = b"\n    machine write_line"
    replacement = b"\n/**/machine write_line"
    require(len(marker) == len(replacement) and marker in comment[portable_label],
            "comment-invariance marker")
    comment[portable_label] = comment[portable_label].replace(marker, replacement, 1)

    portable_reordered = dict(contents)
    portable = portable_reordered[portable_label]
    provider_start = portable.index(b"data ConsoleNativeProvider")
    helper_start = portable.index(b"machine console_write_bytes")
    provider = portable[provider_start:helper_start]
    portable_reordered[portable_label] = provider + portable[:provider_start] + portable[helper_start:]

    target_reordered = dict(contents)
    target = target_reordered[target_label]
    starts = [match.start() for match in re.finditer(rb"(?m)^linux_x64 machine", target)]
    require(len(starts) == 4, "target-leaf invariance source shape")
    starts.append(len(target))
    leaves = [target[start:end] for start, end in zip(starts, starts[1:])]
    target_reordered[target_label] = leaves[1] + leaves[0] + leaves[2] + leaves[3]

    return [
        ("comment-whitespace-invariance", encode_envelope(comment)),
        ("portable-declaration-reorder", encode_envelope(portable_reordered)),
        ("target-leaf-reorder", encode_envelope(target_reordered)),
    ]


def build_outputs(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    contents = fixture_contents()
    envelope = encode_envelope(contents)
    witness = encode_witness(envelope)
    decode_witness(envelope, witness)
    (output / "canonical.omgc").write_bytes(envelope)
    (output / "canonical.omgrsw9").write_bytes(witness)
    (output / "inspection.json").write_text(
        json.dumps(decode_witness(envelope, witness), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    with (output / "positive-cases.tsv").open("w", encoding="utf-8") as rows:
        for name, positive_envelope in positive_variants(contents):
            positive_witness = encode_witness(positive_envelope)
            decode_witness(positive_envelope, positive_witness)
            if name == "comment-whitespace-invariance":
                require(positive_witness == witness,
                        "same-position comment/whitespace changed OMGRSW9 bytes")
            input_path = output / f"{name}.omgc"
            witness_path = output / f"{name}.omgrsw9"
            input_path.write_bytes(positive_envelope)
            witness_path.write_bytes(positive_witness)
            rows.write(f"{name}\t{input_path}\t{witness_path}\n")

    build_label, app_label, portable_label, target_label = LABELS
    build_select = b"builder.select_provider<Console, ConsoleNativeProvider>();"
    cases: list[tuple[str, int, bytes]] = [
        ("wrong-build-source", 251, encode_envelope(contents, 3)),
        ("selection-outside-build", 251, encode_envelope({
            **contents,
            build_label: contents[build_label].replace(build_select, b""),
            app_label: contents[app_label] + b"\n" + build_select + b"\n",
        })),
        ("provider-defaults-not-authority", 251, semantic_case(
            contents, build_label, build_select, b"builder.provider_defaults();")),
        ("unique-candidate-not-authority", 251, semantic_case(
            contents, build_label, build_select, b"")),
        ("missing-plan-row", 251, semantic_case(
            contents, target_label, b"ConsoleNativeProvider::read_line",
            b"ConsoleNativeProvider::read_linx")),
        ("duplicate-plan-row", 251, encode_envelope({
            **contents,
            target_label: contents[target_label] + b"\n" +
                contents[target_label].split(b";", 1)[0] + b";\n",
        })),
        ("mixed-plan-row", 251, semantic_case(
            contents, target_label, b"satisfies Console::read_line",
            b"satisfies Console::write_line")),
        ("wrong-signature", 251, semantic_case(
            contents, target_label, b"write_byte(byte: i32)", b"write_byte(byte: u8)")),
        ("wrong-result", 251, semantic_case(
            contents, target_label, b"read_byte() -> ByteRead", b"read_byte() -> bool")),
        ("wrong-reach", 251, semantic_case(
            contents, portable_label, b"satisfies Console::write\n    reaches Console",
            b"satisfies Console::write\n    reaches ByteRead")),
        ("wrong-rank", 251, semantic_case(
            contents, portable_label, b"Slice::Length", b"Slice::Lenx")),
        ("guarded-head-permutation", 251, semantic_case(
            contents, portable_label, b"emit(console, bytes[0], bytes[1..], newline)",
            b"emit(console, bytes[1], bytes[1..], newline)")),
        ("guarded-tail-permutation", 251, semantic_case(
            contents, portable_label, b"emit(console, bytes[0], bytes[1..], newline)",
            b"emit(console, bytes[0], bytes[0..], newline)")),
        ("adapter-boolean-permutation", 251, semantic_case(
            contents, portable_label, b"console_write_bytes(console, text, false)",
            b"console_write_bytes(console, text, true)")),
        ("requirement-declaration-mismatch", 251, semantic_case(
            contents, portable_label, b"machine write_line(text: &[u8])",
            b"machine write_linx(text: &[u8])")),
        ("build-statement-order", 251, semantic_case(
            contents, build_label,
            b"builder.application(\"omega-bootstrap-provider-plan\");\n"
            b"    builder.select_provider<Console, ConsoleNativeProvider>();",
            b"builder.select_provider<Console, ConsoleNativeProvider>();\n"
            b"    builder.application(\"omega-bootstrap-provider-plan\");")),
        ("build-application-token-swap", 251, semantic_case(
            contents, build_label, b"omega-bootstrap-provider-plan",
            b"omega-bootstrap-provider-plax")),
        ("app-receiver-context", 251, semantic_case(
            contents, app_label, b"self.console.read_byte()", b"self.inputxx.read_byte()")),
        ("wrong-target", 251, semantic_case(
            contents, target_label, b"linux_x64", b"linux_x86", count=4)),
        ("wrong-binding", 251, semantic_case(
            contents, target_label, b"Binding::CompilerIntrinsic", b"Binding::Checked")),
        ("wrong-call-target", 251, semantic_case(
            contents, portable_label, b"console.write_byte(output)",
            b"console.write_line(output)")),
        ("wrong-version", 251, replace_u16(envelope, 8, 2)),
        ("truncated-eof", 251, envelope[:-1]),
        ("trailing-eof", 251, envelope + b"x"),
        ("resource-identifier-adjacent", 252, semantic_case(
            contents, portable_label, b"ConsoleNativeProvider", b"P" * 65)),
        ("resource-input-adjacent", 252, bytes(267_281)),
    ]
    with (output / "resolver-cases.tsv").open("w", encoding="utf-8") as rows:
        for name, status, raw in cases:
            path = output / f"{name}.omgc"
            path.write_bytes(raw)
            rows.write(f"{name}\t{status}\t{path}\n")

    offsets: dict[str, int] = {}
    cursor = WITNESS_HEADER.size
    for name, table in witness_tables(decode_envelope(envelope)[0]):
        offsets[name] = cursor
        cursor += len(table)
    mutations = {
        "magic": b"X" + witness[1:],
        "major": replace_u16(witness, 8, 8),
        "input-extent": replace_u32(witness, 20, len(envelope) + 1),
        "type": replace_u32(witness, offsets["types"] + 5 * TYPE.size + 16, 1),
        "requirement-result": replace_u32(
            witness, offsets["requirements"] + 3 * REQUIREMENT.size + 32, 0),
        "reach": replace_u32(witness, offsets["reaches"] + 4 * REACH.size + 8, 1),
        "helper-rank": replace_u32(witness, offsets["helpers"] + 32, 2),
        "adapter-helper": replace_u32(witness, offsets["adapters"] + 32, 1),
        "candidate-signature": replace_u32(
            witness, offsets["candidate_parameters"] + 4 * PARAMETER.size + 12, 5),
        "candidate-target": replace_u32(
            witness, offsets["candidates"] + 2 * CANDIDATE.size + 48, 0),
        "candidate-binding": replace_u32(
            witness, offsets["candidates"] + 2 * CANDIDATE.size + 52, 1),
        "selection-provenance": replace_u32(witness, offsets["selections"] + 28, 0),
        "plan-incomplete": replace_u32(witness, offsets["plans"] + 32, 0),
        "plan-row-missing": replace_u32(witness, offsets["plans"] + 24, 5),
        "plan-row-duplicate": replace_u32(
            witness, offsets["plan_rows"] + 5 * PLAN_ROW.size + 12, 4),
        "plan-row-mixed": replace_u32(
            witness, offsets["plan_rows"] + 4 * PLAN_ROW.size + 16, 5),
        "requirement-call-target": replace_u32(
            witness, offsets["requirement_calls"] + 4 * REQUIREMENT_CALL.size + 16, 4),
        "ordinary-call-target": replace_u32(
            witness, offsets["ordinary_calls"] + 16, 1),
        "truncated": witness[:-1],
        "trailing": witness + b"x",
    }
    for name, mutated in mutations.items():
        expect_reject(name, 251, lambda raw=mutated: decode_witness(envelope, raw))
    expect_reject("witness-resource-adjacent", 252,
                  lambda: decode_witness(envelope, bytes(524_289)))
    print("OMGRSW9 independent reference: exact 2304-byte witness and mutations PASS")


def check(envelope: Path, witness: Path) -> None:
    print(json.dumps(decode_witness(envelope.read_bytes(), witness.read_bytes()), sort_keys=True))


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
        build_outputs(arguments.output)
    else:
        check(arguments.envelope, arguments.witness)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ReferenceError) as error:
        status = error.status if isinstance(error, ReferenceError) else 251
        print(f"OMGRSW9 independent reference: {error}", file=sys.stderr)
        raise SystemExit(status)
