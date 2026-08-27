#!/usr/bin/env python3
"""Independent OMGCOMP1/OMGRSWA fixture for guarded full-u64 buffers."""

from __future__ import annotations

import argparse
import hashlib
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "compiler"))
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402

NO_ID = 0xFFFF_FFFF
PACKAGE_KEY = "55" * 32
HEADER = struct.Struct("<8sHHHH28I")

CANONICAL = """module app;
data SourceUnit {
    bytes: [u8; 65536] in Trapping;
    length: u64 [0..=65536];
    last_retained: bool;
}
machine SourceUnit::clear(&mut self) {
    self.length = 0;
    self.last_retained = true;
}
machine SourceUnit::append(&mut self, byte: u8) {
    self.last_retained = false;
    transition self.length < 65536 { true -> retain() _ -> full() }
    state retain(&mut self) {
        self.bytes[self.length] = byte;
        self.length = self.length + 1;
        self.last_retained = true;
    }
    state full(&mut self) { self.last_retained = false; }
}
machine SourceUnit::byte_or_nul(&self, index: u64 in Trapping) -> u8 {
    transition index < self.length { true -> retain() _ -> absent() }
    state retain(&self) { self.bytes[index] }
    state absent(&self) { 0 }
}
data Main { source: SourceUnit; observed: u8; absent: u8; }
machine Main::run(&mut self) -> u8 {
    self.source.clear();
    self.source.append(70);
    self.observed = self.source.byte_or_nul(0);
    self.source.length = 65536;
    self.source.append(71);
    self.absent = self.source.byte_or_nul(65536);
    self.observed
}
"""


@dataclass(frozen=True)
class Token:
    value: str
    start: int
    end: int


TOKEN = re.compile(r"//[^\n]*|/\*.*?\*/|[A-Za-z_][A-Za-z0-9_]*|[0-9]+|\.\.=|::|->|.", re.S)


def lex(source: bytes) -> list[Token]:
    text = source.decode("ascii")
    result: list[Token] = []
    for match in TOKEN.finditer(text):
        value = match.group(0)
        if value.isspace() or value.startswith("//") or value.startswith("/*"):
            continue
        result.append(Token(value, match.start(), match.end()))
    return result


def occurrence(tokens: list[Token], values: tuple[str, ...], ordinal: int = 0) -> int:
    hits = [i for i in range(len(tokens) - len(values) + 1)
            if tuple(t.value for t in tokens[i:i + len(values)]) == values]
    if ordinal >= len(hits):
        raise ValueError(f"missing token sequence {values!r} occurrence {ordinal}")
    return hits[ordinal]


def brace_span(tokens: list[Token], start: int) -> tuple[int, int]:
    opening = next(i for i in range(start, len(tokens)) if tokens[i].value == "{")
    depth = 0
    for i in range(opening, len(tokens)):
        depth += tokens[i].value == "{"
        depth -= tokens[i].value == "}"
        if depth == 0:
            return tokens[opening].start, tokens[i].end - tokens[opening].start
    raise ValueError("unclosed body")


def token_span(token: Token) -> tuple[int, int]:
    return token.start, token.end - token.start


def invocation(tokens: list[Token], name: str, ordinal: int) -> tuple[int, int]:
    at = occurrence(tokens, (name, "("), ordinal)
    depth = 0
    for i in range(at + 1, len(tokens)):
        depth += tokens[i].value == "("
        depth -= tokens[i].value == ")"
        if depth == 0:
            return tokens[at].start, tokens[i].end - tokens[at].start
    raise ValueError("unclosed invocation")


def encode_compilation(source: str, *, root_owner: str = "Main",
                       root_machine: str = "run") -> bytes:
    packed = bundle.encode([bundle.Entry("buffer.omg", source.encode("ascii"))])
    manifest = {
        "target": "linux_x86_64",
        "packages": [{
            "key": PACKAGE_KEY,
            "sources": [{"label": "buffer.omg", "module": "app"}],
        }],
        "aliases": [],
        "root": {
            "package": PACKAGE_KEY, "source": "buffer.omg",
            "owner": root_owner, "machine": root_machine,
        },
    }
    return compilation.encode_manifest(manifest, packed)


def encode_witness(envelope: bytes, source: bytes) -> bytes:
    tokens = lex(source)
    names = {name: tokens[occurrence(tokens, prefix)] for name, prefix in {
        "SourceUnit": ("data", "SourceUnit"), "Main": ("data", "Main"),
        "clear": ("machine", "SourceUnit", "::", "clear"),
        "append": ("machine", "SourceUnit", "::", "append"),
        "lookup": ("machine", "SourceUnit", "::", "byte_or_nul"),
        "run": ("machine", "Main", "::", "run"),
    }.items()}
    # sequence() above returns the first token; select each terminal identifier.
    names["SourceUnit"] = tokens[occurrence(tokens, ("data", "SourceUnit")) + 1]
    names["Main"] = tokens[occurrence(tokens, ("data", "Main")) + 1]
    for key, owner, name in (("clear", "SourceUnit", "clear"),
                             ("append", "SourceUnit", "append"),
                             ("lookup", "SourceUnit", "byte_or_nul"),
                             ("run", "Main", "run")):
        names[key] = tokens[occurrence(tokens, ("machine", owner, "::", name)) + 3]

    def named(after: tuple[str, ...], value: str, ordinal: int = 0) -> Token:
        start = occurrence(tokens, after, ordinal) + len(after)
        return next(t for t in tokens[start:] if t.value == value)

    fields = [
        named(("data", "SourceUnit", "{"), "bytes"),
        named(("data", "SourceUnit", "{"), "length"),
        named(("data", "SourceUnit", "{"), "last_retained"),
        named(("data", "Main", "{"), "source"),
        named(("data", "Main", "{"), "observed"),
        named(("data", "Main", "{"), "absent"),
    ]
    byte_param = named(("machine", "SourceUnit", "::", "append", "("), "byte")
    index_param = named(("machine", "SourceUnit", "::", "byte_or_nul", "("), "index")
    machine_starts = [occurrence(tokens, ("machine", "SourceUnit", "::", "clear")),
                      occurrence(tokens, ("machine", "SourceUnit", "::", "append")),
                      occurrence(tokens, ("machine", "SourceUnit", "::", "byte_or_nul")),
                      occurrence(tokens, ("machine", "Main", "::", "run"))]
    machine_bodies = [brace_span(tokens, start) for start in machine_starts]
    state_specs = [(1, "retain", 0), (1, "full", 0),
                   (2, "retain", 1), (2, "absent", 0)]
    states: list[tuple[int, Token, tuple[int, int]]] = []
    for machine, name, ordinal in state_specs:
        at = occurrence(tokens, ("state", name), ordinal)
        states.append((machine, tokens[at + 1], brace_span(tokens, at)))

    types = [
        # id, kind, flags, payload0, payload1, lo.lo, lo.hi, hi.lo, hi.hi
        (0, 0, 0, 0, 0, 0, 0, 0, 0),
        (1, 1, 0, 0, 0, 0, 0, 255, 0),
        (2, 3, 0, 0, 0, 0, 0, 1, 0),
        (3, 10, 1, 0, 0, 0, 0, 0xFFFFFFFF, 0xFFFFFFFF),
        (4, 10, 0, 0, 0, 0, 0, 65536, 0),
        (5, 5, 1, 1, 65536, 0, 0, 0, 0),
        (6, 4, 0, 0, 0, 0, 0, 0, 0),
        (7, 4, 0, 1, 0, 0, 0, 0, 0),
    ]
    payload = bytearray()
    payload += struct.pack("<5I", 0, 0, 0, len(source), 0)
    for row in types:
        ident, kind, flags, p0, p1, lo0, lo1, hi0, hi1 = row
        payload += struct.pack("<IBBH6I", ident, kind, flags, 0,
                               p0, p1, lo0, lo1, hi0, hi1)
    payload += struct.pack("<7I", 0, 0, 6, 0, 3, *token_span(names["SourceUnit"]))
    payload += struct.pack("<7I", 1, 0, 7, 3, 3, *token_span(names["Main"]))
    field_types = (5, 4, 2, 6, 1, 1)
    for ident, (token, owner, ordinal, type_id) in enumerate(zip(
            fields, (0, 0, 0, 1, 1, 1), (0, 1, 2, 0, 1, 2), field_types)):
        payload += struct.pack("<6I", ident, owner, ordinal, type_id, *token_span(token))
    machine_rows = (
        (0, 0, 0, 2, NO_ID, 0, 0, 0, 1, names["clear"], machine_bodies[0]),
        (1, 0, 0, 2, NO_ID, 0, 1, 1, 3, names["append"], machine_bodies[1]),
        (2, 0, 0, 1, 1, 1, 1, 4, 3, names["lookup"], machine_bodies[2]),
        (3, 0, 1, 2, 1, 2, 0, 7, 1, names["run"], machine_bodies[3]),
    )
    for ident, source_id, owner, access, result, ps, pc, bs, bc, name, body in machine_rows:
        payload += struct.pack("<14I", ident, source_id, owner, access, result,
                               ps, pc, bs, bc, *token_span(name), *body, 0)
    payload += struct.pack("<6I", 0, 1, 0, 1, *token_span(byte_param))
    payload += struct.pack("<6I", 1, 2, 0, 3, *token_span(index_param))
    entry_rows = ((0, 0, 2, machine_bodies[0]), (1, 1, 2, machine_bodies[1]),
                  (4, 2, 1, machine_bodies[2]), (7, 3, 2, machine_bodies[3]))
    state_by_block = {2: states[0], 3: states[1], 5: states[2], 6: states[3]}
    entry_by_block = {row[0]: row for row in entry_rows}
    for block in range(8):
        if block in entry_by_block:
            _, machine, access, body = entry_by_block[block]
            ordinal = 0; name_span = (0, 0)
        else:
            machine, name, body = state_by_block[block]
            ordinal = block - (1 if machine == 1 else 4)
            name_span = token_span(name)
            access = 2 if machine == 1 else 1
        payload += struct.pack("<10I", block, machine, ordinal, access,
                               0, 0, *name_span, *body)
    # Occurrence zero is each machine declaration.  Calls begin at one.
    call_specs = ((3, 0, 3, "clear", 1), (3, 1, 3, "append", 1),
                  (3, 2, 3, "byte_or_nul", 1), (3, 1, 3, "append", 2),
                  (3, 2, 3, "byte_or_nul", 2))
    for ident, (caller, target, receiver_field, name, ordinal) in enumerate(call_specs):
        payload += struct.pack("<9I", ident, 0, caller, target, receiver_field,
                               *invocation(tokens, name, ordinal),
                               0 if name == "clear" else 1, 0)
    counts = (1, 8, 2, 6, 4, 2, 8, 5)
    total = HEADER.size + len(payload)
    header = HEADER.pack(b"OMGRSWA\0", 10, 0, 0, 128, total, len(envelope),
                         *counts, 3, 0, 0, 1, 2, 65536,
                         1, 2, 3, 4, 5, 6, 7, 1, 0, 0, 0, 0)
    if total != 1376:
        raise ValueError(f"unexpected OMGRSWA extent {total}")
    return header + payload


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    envelope = encode_compilation(CANONICAL)
    witness = encode_witness(envelope, CANONICAL.encode("ascii"))
    (output / "canonical.omg").write_text(CANONICAL, encoding="ascii")
    (output / "canonical.omgc").write_bytes(envelope)
    (output / "canonical.omgrswa").write_bytes(witness)
    (output / "identity.txt").write_text(
        f"omgc_bytes={len(envelope)}\nomgc_sha256={hashlib.sha256(envelope).hexdigest()}\n"
        f"omgrswa_bytes={len(witness)}\nomgrswa_sha256={hashlib.sha256(witness).hexdigest()}\n",
        encoding="ascii")


def renamed_source() -> str:
    replacements = {
        "SourceUnit": "BufferBox", "Main": "Driver", "bytes": "storage",
        "length": "count", "last_retained": "accepted", "clear": "reset",
        "append": "push", "byte_or_nul": "peek", "byte": "item",
        "index": "position", "retain": "keep", "full": "filled",
        "absent": "missing", "source": "buffer", "observed": "seen",
    }
    source = CANONICAL
    for old, new in replacements.items():
        source = re.sub(rf"\b{old}\b", new, source)
    return source


def reordered_source() -> str:
    source = CANONICAL.replace(
        "    bytes: [u8; 65536] in Trapping;\n"
        "    length: u64 [0..=65536];\n"
        "    last_retained: bool;",
        "    last_retained: bool;\n"
        "    bytes: [u8; 65536] in Trapping;\n"
        "    length: u64 [0..=65536];",
    ).replace(
        "    state retain(&mut self) {\n"
        "        self.bytes[self.length] = byte;\n"
        "        self.length = self.length + 1;\n"
        "        self.last_retained = true;\n"
        "    }\n"
        "    state full(&mut self) { self.last_retained = false; }",
        "    state full(&mut self) { self.last_retained = false; }\n"
        "    state retain(&mut self) {\n"
        "        self.bytes[self.length] = byte;\n"
        "        self.length = self.length + 1;\n"
        "        self.last_retained = true;\n"
        "    }",
    ).replace(
        "    state retain(&self) { self.bytes[index] }\n"
        "    state absent(&self) { 0 }",
        "    state absent(&self) { 0 }\n"
        "    state retain(&self) { self.bytes[index] }",
    )
    tokens = lex(source.encode("ascii"))
    chunks: list[str] = []
    starts: list[int] = []
    for i, token in enumerate(tokens):
        if token.value in ("data", "machine"):
            if token.value == "machine" or (i == 0 or tokens[i - 1].value in (";", "}")):
                starts.append(token.start)
    for start in starts:
        at = next(i for i, token in enumerate(tokens) if token.start == start)
        _, length = brace_span(tokens, at)
        opening = next(i for i in range(at, len(tokens)) if tokens[i].value == "{")
        end = tokens[opening].start + length
        chunks.append(source[start:end])
    prefix = source[:starts[0]]
    by_key = {}
    for chunk in chunks:
        header = chunk.split("{", 1)[0]
        if header.startswith("data "):
            key = "data-" + header.split()[1]
        else:
            key = "machine-" + header.split("::", 1)[1].split("(", 1)[0]
        by_key[key] = chunk
    order = ("data-Main", "machine-run", "machine-byte_or_nul",
             "data-SourceUnit", "machine-clear", "machine-append")
    return prefix + "\n".join(by_key[key] for key in order) + "\n"


def inspect(envelope: bytes, witness: bytes) -> None:
    if len(witness) != 1376:
        raise ValueError("OMGRSWA length")
    header = HEADER.unpack_from(witness)
    if header[:5] != (b"OMGRSWA\0", 10, 0, 0, 128):
        raise ValueError("OMGRSWA identity")
    if header[5] != len(witness) or header[6] != len(envelope):
        raise ValueError("OMGRSWA paired extent")
    if header[7:15] != (1, 8, 2, 6, 4, 2, 8, 5):
        raise ValueError("OMGRSWA counts")
    if header[20] > 65536 or header[28] != 1:
        raise ValueError("OMGRSWA selected relation")
    # Full-u64 lookup policy remains authored; constrained length does not.
    type_start = 128 + 20
    rows = [struct.unpack_from("<IBBH6I", witness, type_start + 32 * i)
            for i in range(8)]
    if rows[3][1:4] != (10, 1, 0) or rows[4][1:4] != (10, 0, 0):
        raise ValueError("OMGRSWA u64 policy custody")


def build_matrix(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    positives = {
        "canonical": (CANONICAL, "Main", "run"),
        "renamed": (renamed_source(), "Driver", "run"),
        "reordered": (reordered_source(), "Main", "run"),
        "inert-field": (CANONICAL.replace(
            "    last_retained: bool;",
            "    id: u8;\n    last_retained: bool;"), "Main", "run"),
        "comments": (CANONICAL.replace(
            "machine SourceUnit::append", "/* independent trivia */\nmachine SourceUnit::append"),
            "Main", "run"),
    }
    positive_lines = []
    for name, (source, owner, machine) in positives.items():
        envelope = encode_compilation(source, root_owner=owner, root_machine=machine)
        path = output / f"{name}.omgc"
        path.write_bytes(envelope)
        positive_lines.append(f"{name}\t{path}\n")
    negatives = {
        "array-65537": CANONICAL.replace("65536", "65537"),
        "missing-index-policy": CANONICAL.replace("index: u64 in Trapping", "index: u64"),
        "computed-index": CANONICAL.replace("self.bytes[index]", "self.bytes[index + 0]"),
        "wrong-increment": CANONICAL.replace("self.length + 1", "self.length + 2"),
        "unguarded-store": CANONICAL.replace("self.bytes[self.length]", "self.bytes[0]"),
    }
    negative_lines = []
    for name, source in negatives.items():
        path = output / f"{name}.omgc"
        path.write_bytes(encode_compilation(source))
        negative_lines.append(f"{name}\t251\t{path}\n")
    trailing = output / "trailing-byte.omgc"
    trailing.write_bytes(encode_compilation(CANONICAL) + b"\0")
    negative_lines.append(f"trailing-byte\t251\t{trailing}\n")
    exhausted = output / "input-exhausted.omgc"
    exhausted.write_bytes(b"\0" * 267282)
    negative_lines.append(f"input-exhausted\t252\t{exhausted}\n")
    (output / "positives.tsv").write_text("".join(positive_lines), encoding="ascii")
    (output / "negatives.tsv").write_text("".join(negative_lines), encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("build", "matrix", "inspect"))
    parser.add_argument("output", type=Path)
    parser.add_argument("witness", nargs="?", type=Path)
    args = parser.parse_args()
    if args.command == "build":
        build(args.output)
    elif args.command == "matrix":
        build_matrix(args.output)
    else:
        inspect(args.output.read_bytes(), args.witness.read_bytes())


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, compilation.CompilationError, bundle.BundleError,
            struct.error) as error:
        raise SystemExit(f"OMGRSWA fixture: {error}")
