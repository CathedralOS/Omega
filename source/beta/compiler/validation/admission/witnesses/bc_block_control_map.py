#!/usr/bin/env python3
"""Build an untrusted location witness for bc.beta's emitted Alpha control graph.

The resulting witness contains no source or tape bytes.  This program is
deliberately outside the trusted claim: it may suggest source-block and source
transition locations, but the gate supplies the exact source/artifact and the
Alpha checker re-lexes and decodes them itself.
"""

from __future__ import annotations

import argparse
import importlib.util
import struct
from dataclasses import dataclass
from pathlib import Path


OPS = {
    "halt": (0x00, "r"), "imm": (0x01, "rx"),
    "mov": (0x02, "rr"), "add": (0x03, "rr"),
    "sub": (0x04, "rr"), "mul": (0x05, "rr"),
    "div": (0x06, "rr"), "mod": (0x07, "rr"),
    "loadb": (0x08, "rr"), "storeb": (0x09, "rr"),
    "load": (0x0A, "rr"), "store": (0x0B, "rr"),
    "jmp": (0x0C, "x"), "jz": (0x0D, "rx"),
    "jnz": (0x0E, "rx"), "jlt": (0x0F, "rrx"),
    "jeq": (0x10, "rrx"), "read": (0x11, "r"),
    "write": (0x12, "r"), "call": (0x13, "x"), "ret": (0x14, ""),
}
ESC = {"n": 10, "t": 9, "r": 13, "0": 0, "\\": 92, "'": 39, '"': 34}
MAGIC = 0x31314342  # little-endian "BC11"

EVENT_CALL = 1
EVENT_READ = 2
EVENT_WRITE = 3
EVENT_EMIT = 4
EVENT_RETURN = 5
ACCESS_LOAD = 1
ACCESS_STORE = 2
MEMORY_LOAD = 1
MEMORY_STORE = 2
PRIMITIVE_LITERAL = 1
PRIMITIVE_ARITHMETIC = 2
PRIMITIVE_COMPARISON = 3
PUSH_BINARY_LEFT = 1
PUSH_CALL_ARGUMENT = 2
PUSH_STORE_ADDRESS = 3


@dataclass(frozen=True)
class Block:
    proc_index: int
    proc_name: str
    name: str
    label: str
    transitions: tuple[tuple[str, bool], ...]


@dataclass(frozen=True)
class Item:
    kind: str
    name: str
    operands: tuple[str, ...]
    offset: int
    size: int


@dataclass(frozen=True)
class Event:
    kind: int
    name: str
    literal: bytes
    node_id: int
    block_index: int
    arity: int


@dataclass(frozen=True)
class LocalAccess:
    kind: int
    slot: int
    node_id: int
    block_index: int


@dataclass(frozen=True)
class MemorySite:
    kind: int
    width: int
    node_id: int
    block_index: int


@dataclass(frozen=True)
class ExprPrimitive:
    kind: int
    value: int
    node_id: int
    block_index: int


@dataclass(frozen=True)
class StackPush:
    kind: int
    node_id: int
    ordinal: int
    block_index: int


def load_parser(repo: Path):
    path = repo / "source/beta/reference/beta_parser.py"
    spec = importlib.util.spec_from_file_location("bc_control_beta_parser", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load parser at {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def source_blocks(repo: Path, source: bytes) -> list[Block]:
    parser = load_parser(repo)
    ast = parser.Parser(parser.lex(source.decode("utf-8"))).parse()
    blocks: list[Block] = []
    for proc_index, proc in enumerate(ast):
        _, proc_name, _params, body = proc
        entry_transitions = tuple(
            (stmt[1], stmt[2] is not None) for stmt in body if stmt[0] == "goto"
        )
        blocks.append(Block(proc_index, proc_name, proc_name, proc_name,
                            entry_transitions))
        for stmt in body:
            if stmt[0] != "state":
                continue
            _, state_name, state_body = stmt
            transitions = tuple(
                (inner[1], inner[2] is not None)
                for inner in state_body if inner[0] == "goto"
            )
            blocks.append(Block(proc_index, proc_name, state_name,
                                f"{proc_name}__{state_name}", transitions))
    return blocks


def source_events(repo: Path, source: bytes):
    """Return lexical events plus each procedure's lowering-order events.

    The witness is canonical by source order.  Calls lower after their arguments,
    so the second traversal retains node identity while recovering artifact order.
    Both traversals are untrusted hints; the Alpha checker independently scans the
    source sites and validates every suggested instruction.
    """
    parser = load_parser(repo)
    ast = parser.Parser(parser.lex(source.decode("utf-8"))).parse()
    lexical: list[Event] = []
    event_by_node: dict[int, Event] = {}
    lowering_by_proc: dict[int, list[Event]] = {}
    accesses: list[LocalAccess] = []
    access_by_node: dict[int, LocalAccess] = {}
    access_lowering_by_proc: dict[int, list[LocalAccess]] = {}
    memory_sites: list[MemorySite] = []
    memory_by_node: dict[int, MemorySite] = {}
    memory_lowering_by_proc: dict[int, list[MemorySite]] = {}
    primitives: list[ExprPrimitive] = []
    primitive_by_node: dict[int, ExprPrimitive] = {}
    primitive_lowering_by_proc: dict[int, list[ExprPrimitive]] = {}
    binary_pushes: list[StackPush] = []
    argument_pushes: list[StackPush] = []
    store_pushes: list[StackPush] = []
    push_by_key: dict[tuple[int, int, int], StackPush] = {}
    push_lowering_by_proc: dict[int, list[StackPush]] = {}
    block_index = 0
    current_slots: dict[str, int] = {}

    def add(kind: int, name: str, literal: bytes, node, block: int,
            arity: int = 0) -> None:
        event = Event(kind, name, literal, id(node), block, arity)
        lexical.append(event)
        event_by_node[id(node)] = event
        if kind == EVENT_CALL:
            for ordinal in range(arity):
                push = StackPush(PUSH_CALL_ARGUMENT, id(node), ordinal, block)
                argument_pushes.append(push)
                push_by_key[(push.kind, push.node_id, push.ordinal)] = push

    def lex_expr(expr, block: int) -> None:
        if expr[0] == "call":
            name = expr[1]
            kind = EVENT_READ if name == "read_byte" else (
                EVENT_WRITE if name == "write_byte" else EVENT_CALL
            )
            add(kind, name, b"", expr, block, len(expr[2]))
            for argument in expr[2]:
                lex_expr(argument, block)
        elif expr[0] == "bin":
            lex_expr(expr[2], block)
            lex_expr(expr[3], block)
        elif expr[0] == "mem":
            lex_expr(expr[2], block)

    def add_access(kind: int, slot: int, node, block: int) -> None:
        access = LocalAccess(kind, slot, id(node), block)
        accesses.append(access)
        access_by_node[id(node)] = access

    def lex_access_expr(expr, block: int) -> None:
        if expr[0] == "var":
            add_access(ACCESS_LOAD, current_slots[expr[1]], expr, block)
        elif expr[0] == "call":
            for argument in expr[2]:
                lex_access_expr(argument, block)
        elif expr[0] == "bin":
            lex_access_expr(expr[2], block)
            lex_access_expr(expr[3], block)
        elif expr[0] == "mem":
            lex_access_expr(expr[2], block)

    def add_memory(kind: int, width: int, node, block: int) -> None:
        site = MemorySite(kind, width, id(node), block)
        memory_sites.append(site)
        memory_by_node[id(node)] = site
        if kind == MEMORY_STORE:
            push = StackPush(PUSH_STORE_ADDRESS, id(node), 0, block)
            store_pushes.append(push)
            push_by_key[(push.kind, push.node_id, push.ordinal)] = push

    def lex_memory_expr(expr, block: int) -> None:
        if expr[0] == "mem":
            add_memory(MEMORY_LOAD, 1 if expr[1] == "byte" else 8,
                       expr, block)
            lex_memory_expr(expr[2], block)
        elif expr[0] == "call":
            for argument in expr[2]:
                lex_memory_expr(argument, block)
        elif expr[0] == "bin":
            lex_memory_expr(expr[2], block)
            lex_memory_expr(expr[3], block)

    arithmetic_codes = {"+": 3, "-": 4, "*": 5, "/": 6, "%": 7}
    comparison_codes = {
        "<": 8, ">": 9, "==": 10, "!=": 11, "<=": 12, ">=": 13,
    }

    def add_primitive(kind: int, value: int, node, block: int) -> None:
        primitive = ExprPrimitive(kind, value, id(node), block)
        primitives.append(primitive)
        primitive_by_node[id(node)] = primitive
        if kind in (PRIMITIVE_ARITHMETIC, PRIMITIVE_COMPARISON):
            push = StackPush(PUSH_BINARY_LEFT, id(node), 0, block)
            binary_pushes.append(push)
            push_by_key[(push.kind, push.node_id, push.ordinal)] = push

    def lex_primitive_expr(expr, block: int) -> None:
        if expr[0] == "num":
            add_primitive(PRIMITIVE_LITERAL, expr[1], expr, block)
        elif expr[0] == "bin":
            lex_primitive_expr(expr[2], block)
            if expr[1] in arithmetic_codes:
                add_primitive(PRIMITIVE_ARITHMETIC,
                              arithmetic_codes[expr[1]], expr, block)
            elif expr[1] in comparison_codes:
                add_primitive(PRIMITIVE_COMPARISON,
                              comparison_codes[expr[1]], expr, block)
            lex_primitive_expr(expr[3], block)
        elif expr[0] == "call":
            for argument in expr[2]:
                lex_primitive_expr(argument, block)
        elif expr[0] == "mem":
            lex_primitive_expr(expr[2], block)

    def lex_stmt(stmt, block: int) -> None:
        kind = stmt[0]
        if kind in ("let", "assign"):
            lex_expr(stmt[2], block)
        elif kind == "return":
            add(EVENT_RETURN, "", b"", stmt, block)
            lex_expr(stmt[1], block)
        elif kind == "goto":
            if stmt[2] is not None:
                lex_expr(stmt[2], block)
        elif kind == "memset":
            lex_expr(stmt[2], block)
            lex_expr(stmt[3], block)
        elif kind == "emit":
            literal = decoded_string('"' + stmt[1] + '"')
            add(EVENT_EMIT, "emit", literal, stmt, block)
        elif kind == "callstmt":
            lex_expr(stmt[1], block)

    def lower_expr(expr, output: list[Event]) -> None:
        if expr[0] == "call":
            for argument in expr[2]:
                lower_expr(argument, output)
            output.append(event_by_node[id(expr)])
        elif expr[0] == "bin":
            lower_expr(expr[2], output)
            lower_expr(expr[3], output)
        elif expr[0] == "mem":
            lower_expr(expr[2], output)

    def lower_access_expr(expr, output: list[LocalAccess]) -> None:
        if expr[0] == "var":
            output.append(access_by_node[id(expr)])
        elif expr[0] == "call":
            for argument in expr[2]:
                lower_access_expr(argument, output)
        elif expr[0] == "bin":
            lower_access_expr(expr[2], output)
            lower_access_expr(expr[3], output)
        elif expr[0] == "mem":
            lower_access_expr(expr[2], output)

    def lower_memory_expr(expr, output: list[MemorySite]) -> None:
        if expr[0] == "mem":
            lower_memory_expr(expr[2], output)
            output.append(memory_by_node[id(expr)])
        elif expr[0] == "call":
            for argument in expr[2]:
                lower_memory_expr(argument, output)
        elif expr[0] == "bin":
            lower_memory_expr(expr[2], output)
            lower_memory_expr(expr[3], output)

    def lower_primitive_expr(expr, output: list[ExprPrimitive]) -> None:
        if expr[0] == "num":
            output.append(primitive_by_node[id(expr)])
        elif expr[0] == "bin":
            lower_primitive_expr(expr[2], output)
            lower_primitive_expr(expr[3], output)
            if expr[1] in arithmetic_codes:
                output.append(primitive_by_node[id(expr)])
            elif expr[1] in comparison_codes:
                output.append(primitive_by_node[id(expr)])
        elif expr[0] == "call":
            for argument in expr[2]:
                lower_primitive_expr(argument, output)
        elif expr[0] == "mem":
            lower_primitive_expr(expr[2], output)

    def lower_push_expr(expr, output: list[StackPush]) -> None:
        if expr[0] == "call":
            event = event_by_node[id(expr)]
            for ordinal, argument in enumerate(expr[2]):
                lower_push_expr(argument, output)
                if event.kind == EVENT_CALL:
                    output.append(push_by_key[
                        (PUSH_CALL_ARGUMENT, id(expr), ordinal)
                    ])
        elif expr[0] == "bin":
            lower_push_expr(expr[2], output)
            output.append(push_by_key[(PUSH_BINARY_LEFT, id(expr), 0)])
            lower_push_expr(expr[3], output)
        elif expr[0] == "mem":
            lower_push_expr(expr[2], output)

    def lower_stmt(stmt, output: list[Event]) -> None:
        kind = stmt[0]
        if kind in ("let", "assign"):
            lower_expr(stmt[2], output)
        elif kind == "return":
            lower_expr(stmt[1], output)
            output.append(event_by_node[id(stmt)])
        elif kind == "goto":
            if stmt[2] is not None:
                lower_expr(stmt[2], output)
        elif kind == "memset":
            lower_expr(stmt[2], output)
            lower_expr(stmt[3], output)
        elif kind == "emit":
            output.append(event_by_node[id(stmt)])
        elif kind == "callstmt":
            lower_expr(stmt[1], output)

    for proc_index, proc in enumerate(ast):
        current_slots = {name: index for index, name in enumerate(proc[2])}
        lowering: list[Event] = []
        access_lowering: list[LocalAccess] = []
        memory_lowering: list[MemorySite] = []
        primitive_lowering: list[ExprPrimitive] = []
        push_lowering: list[StackPush] = []
        entry_block = block_index
        block_index += 1

        def access_stmt(stmt, block: int) -> None:
            kind = stmt[0]
            if kind == "let":
                slot = len(current_slots)
                current_slots[stmt[1]] = slot
                add_access(ACCESS_STORE, slot, stmt, block)
                lex_access_expr(stmt[2], block)
                lower_access_expr(stmt[2], access_lowering)
                access_lowering.append(access_by_node[id(stmt)])
            elif kind == "assign":
                add_access(ACCESS_STORE, current_slots[stmt[1]], stmt, block)
                lex_access_expr(stmt[2], block)
                lower_access_expr(stmt[2], access_lowering)
                access_lowering.append(access_by_node[id(stmt)])
            elif kind == "return":
                lex_access_expr(stmt[1], block)
                lower_access_expr(stmt[1], access_lowering)
            elif kind == "goto" and stmt[2] is not None:
                lex_access_expr(stmt[2], block)
                lower_access_expr(stmt[2], access_lowering)
            elif kind == "memset":
                lex_access_expr(stmt[2], block)
                lex_access_expr(stmt[3], block)
                lower_access_expr(stmt[2], access_lowering)
                lower_access_expr(stmt[3], access_lowering)
            elif kind == "callstmt":
                lex_access_expr(stmt[1], block)
                lower_access_expr(stmt[1], access_lowering)

        def memory_stmt(stmt, block: int) -> None:
            kind = stmt[0]
            if kind in ("let", "assign"):
                lex_memory_expr(stmt[2], block)
                lower_memory_expr(stmt[2], memory_lowering)
            elif kind == "return":
                lex_memory_expr(stmt[1], block)
                lower_memory_expr(stmt[1], memory_lowering)
            elif kind == "goto" and stmt[2] is not None:
                lex_memory_expr(stmt[2], block)
                lower_memory_expr(stmt[2], memory_lowering)
            elif kind == "memset":
                add_memory(MEMORY_STORE, 1 if stmt[1] == "byte" else 8,
                           stmt, block)
                lex_memory_expr(stmt[2], block)
                lex_memory_expr(stmt[3], block)
                lower_memory_expr(stmt[2], memory_lowering)
                lower_memory_expr(stmt[3], memory_lowering)
                memory_lowering.append(memory_by_node[id(stmt)])
            elif kind == "callstmt":
                lex_memory_expr(stmt[1], block)
                lower_memory_expr(stmt[1], memory_lowering)

        def primitive_stmt(stmt, block: int) -> None:
            kind = stmt[0]
            expressions = []
            if kind in ("let", "assign"):
                expressions = [stmt[2]]
            elif kind == "return":
                expressions = [stmt[1]]
            elif kind == "goto" and stmt[2] is not None:
                expressions = [stmt[2]]
            elif kind == "memset":
                expressions = [stmt[2], stmt[3]]
            elif kind == "callstmt":
                expressions = [stmt[1]]
            for expression in expressions:
                lex_primitive_expr(expression, block)
                lower_primitive_expr(expression, primitive_lowering)

        def push_stmt(stmt) -> None:
            kind = stmt[0]
            if kind in ("let", "assign"):
                lower_push_expr(stmt[2], push_lowering)
            elif kind == "return":
                lower_push_expr(stmt[1], push_lowering)
            elif kind == "goto" and stmt[2] is not None:
                lower_push_expr(stmt[2], push_lowering)
            elif kind == "memset":
                lower_push_expr(stmt[2], push_lowering)
                push_lowering.append(
                    push_by_key[(PUSH_STORE_ADDRESS, id(stmt), 0)]
                )
                lower_push_expr(stmt[3], push_lowering)
            elif kind == "callstmt":
                lower_push_expr(stmt[1], push_lowering)

        for stmt in proc[3]:
            if stmt[0] == "state":
                state_block = block_index
                block_index += 1
                for inner in stmt[2]:
                    primitive_stmt(inner, state_block)
                    memory_stmt(inner, state_block)
                    access_stmt(inner, state_block)
                    lex_stmt(inner, state_block)
                    lower_stmt(inner, lowering)
                    push_stmt(inner)
            else:
                primitive_stmt(stmt, entry_block)
                memory_stmt(stmt, entry_block)
                access_stmt(stmt, entry_block)
                lex_stmt(stmt, entry_block)
                lower_stmt(stmt, lowering)
                push_stmt(stmt)
        lowering_by_proc[proc_index] = lowering
        access_lowering_by_proc[proc_index] = access_lowering
        memory_lowering_by_proc[proc_index] = memory_lowering
        primitive_lowering_by_proc[proc_index] = primitive_lowering
        push_lowering_by_proc[proc_index] = push_lowering
    pushes = binary_pushes + argument_pushes + store_pushes
    return (ast, lexical, lowering_by_proc, accesses,
            access_lowering_by_proc, memory_sites, memory_lowering_by_proc,
            primitives, primitive_lowering_by_proc, pushes,
            push_lowering_by_proc)


def same_valued_composition_pair(ast: list,
                                 primitives: list[ExprPrimitive]) -> tuple[int, int]:
    """Find two identical literal records nested in one source expression.

    Swapping their location hints preserves every flat primitive property
    (kind, value, source block, uniqueness, and exact artifact bytes).  Only the
    grammar-directed composition phase can distinguish their roles relative to
    the intervening push/operator pieces.
    """
    primitive_index = {primitive.node_id: index
                       for index, primitive in enumerate(primitives)}

    def walk_expr(expr):
        yield expr
        if expr[0] == "bin":
            yield from walk_expr(expr[2])
            yield from walk_expr(expr[3])
        elif expr[0] == "call":
            for argument in expr[2]:
                yield from walk_expr(argument)
        elif expr[0] == "mem":
            yield from walk_expr(expr[2])

    def stmt_exprs(stmt):
        kind = stmt[0]
        if kind in ("let", "assign"):
            return (stmt[2],)
        if kind == "return":
            return (stmt[1],)
        if kind == "goto" and stmt[2] is not None:
            return (stmt[2],)
        if kind == "memset":
            return (stmt[2], stmt[3])
        if kind == "callstmt":
            return (stmt[1],)
        return ()

    for proc in ast:
        for outer in proc[3]:
            statements = outer[2] if outer[0] == "state" else (outer,)
            for stmt in statements:
                for expr in stmt_exprs(stmt):
                    by_value: dict[int, list[int]] = {}
                    for node in walk_expr(expr):
                        if node[0] != "num":
                            continue
                        index = primitive_index[id(node)]
                        by_value.setdefault(node[1], []).append(index)
                    for indices in by_value.values():
                        if len(indices) < 2:
                            continue
                        left, right = indices[:2]
                        if primitives[left].block_index != primitives[right].block_index:
                            continue
                        return left, right
    raise ValueError("no same-valued literals nested in one source expression")


def strip_comment(line: str) -> str:
    out: list[str] = []
    quoted = False
    for char in line:
        if char == '"':
            quoted = not quoted
        elif char == ";" and not quoted:
            break
        out.append(char)
    return "".join(out)


def tokens(line: str) -> list[str]:
    result: list[str] = []
    i = 0
    while i < len(line):
        if line[i] in " \t\r,":
            i += 1
            continue
        if line[i] == '"':
            j = i + 1
            while j < len(line) and line[j] != '"':
                j += 2 if line[j] == "\\" else 1
            result.append(line[i:j + 1])
            i = j + 1
            continue
        j = i
        while j < len(line) and line[j] not in " \t\r,":
            j += 1
        result.append(line[i:j])
        i = j
    return result


def decoded_string(token: str) -> bytes:
    out = bytearray()
    i = 1
    while i < len(token) - 1:
        if token[i] == "\\":
            out.append(ESC[token[i + 1]])
            i += 2
        else:
            out.append(ord(token[i]))
            i += 1
    return bytes(out)


def parse_alpha(text: str) -> tuple[list[Item], dict[str, int]]:
    raw: list[tuple[str, str, tuple[str, ...], int]] = []
    offset = 0
    labels: dict[str, int] = {}
    for line in text.splitlines():
        line_tokens = tokens(strip_comment(line))
        k = 0
        while k < len(line_tokens):
            token = line_tokens[k]
            if token.endswith(":"):
                labels[token[:-1]] = offset
                raw.append(("label", token[:-1], (), 0))
                k += 1
                continue
            if token == "db":
                payload = decoded_string(line_tokens[k + 1])
                raw.append(("db", "db", (), len(payload)))
                offset += len(payload)
                k += 2
                continue
            if token not in OPS:
                raise ValueError(f"unknown Alpha mnemonic {token!r}")
            kinds = OPS[token][1]
            operands = tuple(line_tokens[k + 1:k + 1 + len(kinds)])
            size = 1 + sum(1 if kind == "r" else 8 for kind in kinds)
            raw.append(("ins", token, operands, size))
            offset += size
            k += 1 + len(kinds)

    items: list[Item] = []
    offset = 0
    for kind, name, operands, size in raw:
        items.append(Item(kind, name, operands, offset, size))
        offset += size
    return items, labels


def locate(blocks: list[Block], assembly: str):
    items, labels = parse_alpha(assembly)
    missing = [block.label for block in blocks if block.label not in labels]
    if missing:
        raise ValueError(f"missing source labels in emitted Alpha: {missing[:4]}")

    block_pcs = [labels[block.label] for block in blocks]
    transition_pcs: list[int] = []
    transition_jmps: list[int] = []
    target_indices: list[int] = []
    guarded_count = 0
    for block_index, block in enumerate(blocks):
        start = labels[block.label]
        end = block_pcs[block_index + 1] if block_index + 1 < len(blocks) else 1 << 62
        candidates = [item for item in items
                      if item.kind == "ins" and item.name == "jmp"
                      and start <= item.offset < end]
        cursor = 0
        for target_name, guarded in block.transitions:
            target_label = f"{block.proc_name}__{target_name}"
            while cursor < len(candidates) and candidates[cursor].operands[0] != target_label:
                cursor += 1
            if cursor == len(candidates):
                raise ValueError(f"cannot locate transition {block.label} -> {target_label}")
            jump = candidates[cursor]
            cursor += 1
            target_index = next(
                (i for i, candidate in enumerate(blocks)
                 if candidate.proc_index == block.proc_index and candidate.name == target_name),
                None,
            )
            if target_index is None:
                raise ValueError(f"source target {target_label} does not name a state")
            if guarded:
                previous = next((item for item in items
                                 if item.kind == "ins" and item.offset + item.size == jump.offset), None)
                if previous is None or previous.name != "jz" or previous.operands[0] != "r0":
                    raise ValueError(f"guarded transition at {jump.offset} lacks adjacent jz r0")
                transition_pcs.append(previous.offset)
                guarded_count += 1
            else:
                transition_pcs.append(jump.offset)
            transition_jmps.append(jump.offset)
            target_indices.append(target_index)
    return (items, labels, block_pcs, transition_pcs, transition_jmps,
            target_indices, guarded_count)


def locate_events(ast: list, lexical: list[Event], lowering_by_proc: dict[int, list[Event]],
                  items: list[Item], labels: dict[str, int], tape_len: int) -> tuple[list[int], int]:
    by_node = {event.node_id: index for index, event in enumerate(lexical)}
    pcs = [-1] * len(lexical)
    proc_starts = [labels[proc[1]] for proc in ast]
    for proc_index, proc in enumerate(ast):
        start = proc_starts[proc_index]
        end = proc_starts[proc_index + 1] if proc_index + 1 < len(proc_starts) else tape_len
        candidates = [
            item for item in items
            if item.kind == "ins" and start <= item.offset < end
            and item.name in {"call", "read", "write", "ret"}
        ]
        expected = lowering_by_proc[proc_index]
        if len(candidates) != len(expected) + 1:
            raise ValueError(
                f"{proc[1]} effect accounting: {len(candidates)} instructions "
                f"for {len(expected)} source events plus fallthrough ret"
            )
        if candidates[-1].name != "ret" or candidates[-1].offset + 1 != end:
            raise ValueError(f"{proc[1]} lacks canonical final fallthrough ret")
        for event, item in zip(expected, candidates[:-1]):
            if event.kind == EVENT_CALL:
                valid = item.name == "call" and item.operands == (event.name,)
            elif event.kind == EVENT_READ:
                valid = item.name == "read" and item.operands == ("r0",)
            elif event.kind == EVENT_WRITE:
                valid = item.name == "write" and item.operands == ("r0",)
            elif event.kind == EVENT_EMIT:
                valid = item.name == "call" and item.operands == ("__write_str",)
            else:
                valid = item.name == "ret"
            if not valid:
                raise ValueError(
                    f"{proc[1]} source event {event} does not match Alpha item {item}"
                )
            pcs[by_node[event.node_id]] = item.offset
    if any(pc < 0 for pc in pcs):
        raise ValueError("not every source effect site received an Alpha location")
    return pcs, labels["__write_str"]


def locate_local_accesses(ast: list, lexical: list[LocalAccess],
                          lowering_by_proc: dict[int, list[LocalAccess]],
                          items: list[Item], labels: dict[str, int],
                          tape_len: int) -> list[int]:
    by_node = {access.node_id: index for index, access in enumerate(lexical)}
    pcs = [-1] * len(lexical)
    proc_starts = [labels[proc[1]] for proc in ast]
    for proc_index, proc in enumerate(ast):
        start = proc_starts[proc_index]
        end = proc_starts[proc_index + 1] if proc_index + 1 < len(proc_starts) else tape_len
        ins = [item for item in items
               if item.kind == "ins" and start <= item.offset < end]
        candidates: list[tuple[Item, int, int]] = []
        for index in range(len(ins) - 3):
            a, b, c, d = ins[index:index + 4]
            if not (a.offset + a.size == b.offset
                    and b.offset + b.size == c.offset
                    and c.offset + c.size == d.offset):
                continue
            if (a.name == "mov" and a.operands == ("r0", "r14")
                    and b.name == "imm" and b.operands[0] == "r2"
                    and c.name == "sub" and c.operands == ("r0", "r2")
                    and d.name == "load" and d.operands == ("r0", "r0")):
                candidates.append((a, ACCESS_LOAD, int(b.operands[1])))
            elif (a.name == "mov" and a.operands == ("r1", "r14")
                  and b.name == "imm" and b.operands[0] == "r2"
                  and c.name == "sub" and c.operands == ("r1", "r2")
                  and d.name == "store" and d.operands == ("r1", "r0")):
                candidates.append((a, ACCESS_STORE, int(b.operands[1])))
        expected = lowering_by_proc[proc_index]
        if len(candidates) != len(expected):
            raise ValueError(
                f"{proc[1]} local access accounting: {len(candidates)} macros "
                f"for {len(expected)} source accesses"
            )
        for access, (item, kind, offset) in zip(expected, candidates):
            if kind != access.kind or offset != 8 + 8 * access.slot:
                raise ValueError(
                    f"{proc[1]} source local access {access} does not match "
                    f"Alpha macro {(item, kind, offset)}"
                )
            pcs[by_node[access.node_id]] = item.offset
    if any(pc < 0 for pc in pcs):
        raise ValueError("not every source local access received an Alpha location")
    return pcs


def locate_memory_sites(ast: list, lexical: list[MemorySite],
                        lowering_by_proc: dict[int, list[MemorySite]],
                        items: list[Item], labels: dict[str, int], tape_len: int,
                        local_access_pcs: list[int]) -> list[int]:
    by_node = {site.node_id: index for index, site in enumerate(lexical)}
    pcs = [-1] * len(lexical)
    proc_starts = [labels[proc[1]] for proc in ast]
    local_final = {pc + 16 for pc in local_access_pcs}
    for proc_index, proc in enumerate(ast):
        start = proc_starts[proc_index]
        end = proc_starts[proc_index + 1] if proc_index + 1 < len(proc_starts) else tape_len
        candidates: list[tuple[Item, int, int]] = []
        for item in items:
            if item.kind != "ins" or not start <= item.offset < end:
                continue
            if (item.name == "loadb" and item.operands == ("r0", "r0")):
                candidates.append((item, MEMORY_LOAD, 1))
            elif (item.name == "load" and item.operands == ("r0", "r0")
                  and item.offset not in local_final):
                candidates.append((item, MEMORY_LOAD, 8))
            elif item.name == "storeb" and item.operands == ("r1", "r0"):
                candidates.append((item, MEMORY_STORE, 1))
            elif (item.name == "store" and item.operands == ("r1", "r0")
                  and item.offset not in local_final):
                candidates.append((item, MEMORY_STORE, 8))
        expected = lowering_by_proc[proc_index]
        if len(candidates) != len(expected):
            raise ValueError(
                f"{proc[1]} raw memory accounting: {len(candidates)} sites "
                f"for {len(expected)} source operations"
            )
        for site, (item, kind, width) in zip(expected, candidates):
            if kind != site.kind or width != site.width:
                raise ValueError(
                    f"{proc[1]} source memory site {site} does not match "
                    f"Alpha instruction {(item, kind, width)}"
                )
            pcs[by_node[site.node_id]] = item.offset
    if any(pc < 0 for pc in pcs):
        raise ValueError("not every source raw memory site received an Alpha location")
    return pcs


def locate_expr_primitives(ast: list, lexical: list[ExprPrimitive],
                           lowering_by_proc: dict[int, list[ExprPrimitive]],
                           items: list[Item], labels: dict[str, int],
                           tape_len: int) -> list[int]:
    by_node = {primitive.node_id: index
               for index, primitive in enumerate(lexical)}
    pcs = [-1] * len(lexical)
    proc_starts = [labels[proc[1]] for proc in ast]
    for proc_index, proc in enumerate(ast):
        start = proc_starts[proc_index]
        end = proc_starts[proc_index + 1] if proc_index + 1 < len(proc_starts) else tape_len
        ins = [item for item in items
               if item.kind == "ins" and start <= item.offset < end]
        ins_by_offset = {item.offset: item for item in ins}
        comparison_ranges: list[tuple[int, int]] = []
        arithmetic: list[tuple[int, int]] = []
        comparisons: list[tuple[int, int]] = []
        for index in range(len(ins) - 3):
            a, b, c, d = ins[index:index + 4]
            base = (
                a.name == "mov" and a.operands == ("r1", "r0")
                and b.name == "load" and b.operands == ("r0", "r15")
                and c.name == "add" and c.operands == ("r15", "r13")
                and a.offset + 3 == b.offset
                and b.offset + 3 == c.offset
                and c.offset + 3 == d.offset
            )
            if not base:
                continue
            if d.name in {"add", "sub", "mul", "div", "mod"} \
                    and d.operands == ("r0", "r1"):
                arithmetic.append((a.offset, OPS[d.name][0]))
            elif d.name in {"jlt", "jeq"}:
                comparison_ranges.append((a.offset, a.offset + 49))
                fall = ins_by_offset.get(a.offset + 20)
                taken = ins_by_offset.get(a.offset + 39)
                if not (fall and taken and fall.name == "imm"
                        and taken.name == "imm"
                        and fall.operands[0] == "r0"
                        and taken.operands[0] == "r0"):
                    raise ValueError(
                        f"{proc[1]} malformed comparison results at {a.offset}"
                    )
                shape = (d.name, d.operands[0], d.operands[1],
                         int(fall.operands[1]), int(taken.operands[1]))
                code_by_shape = {
                    ("jlt", "r0", "r1", 0, 1): 8,
                    ("jlt", "r1", "r0", 0, 1): 9,
                    ("jeq", "r0", "r1", 0, 1): 10,
                    ("jeq", "r0", "r1", 1, 0): 11,
                    ("jlt", "r1", "r0", 1, 0): 12,
                    ("jlt", "r0", "r1", 1, 0): 13,
                }
                if shape not in code_by_shape:
                    raise ValueError(
                        f"{proc[1]} unknown comparison shape {shape}"
                    )
                comparisons.append((a.offset, code_by_shape[shape]))

        candidates: list[tuple[int, int, int]] = [
            (pc, PRIMITIVE_ARITHMETIC, opcode) for pc, opcode in arithmetic
        ]
        candidates.extend(
            (pc, PRIMITIVE_COMPARISON, code) for pc, code in comparisons
        )
        for item in ins:
            if item.name != "imm" or item.operands[0] != "r0":
                continue
            try:
                value = int(item.operands[1])
            except ValueError:
                continue
            if any(lo <= item.offset < hi for lo, hi in comparison_ranges):
                continue
            candidates.append((item.offset, PRIMITIVE_LITERAL, value))
        candidates.sort()
        expected = lowering_by_proc[proc_index]
        if len(candidates) != len(expected):
            raise ValueError(
                f"{proc[1]} expression primitive accounting: "
                f"{len(candidates)} sites for {len(expected)} source primitives"
            )
        for primitive, (pc, kind, value) in zip(expected, candidates):
            if kind != primitive.kind or value != primitive.value:
                raise ValueError(
                    f"{proc[1]} source primitive {primitive} does not match "
                    f"Alpha site {(pc, kind, value)}"
                )
            pcs[by_node[primitive.node_id]] = pc
    if any(pc < 0 for pc in pcs):
        raise ValueError("not every source expression primitive received an Alpha location")
    return pcs


def locate_stack_pushes(ast: list, lexical: list[StackPush],
                        lowering_by_proc: dict[int, list[StackPush]],
                        items: list[Item], labels: dict[str, int],
                        tape_len: int) -> list[int]:
    by_key = {(push.kind, push.node_id, push.ordinal): index
              for index, push in enumerate(lexical)}
    pcs = [-1] * len(lexical)
    proc_starts = [labels[proc[1]] for proc in ast]
    for proc_index, proc in enumerate(ast):
        start = proc_starts[proc_index]
        end = (proc_starts[proc_index + 1]
               if proc_index + 1 < len(proc_starts) else tape_len)
        ins = [item for item in items
               if item.kind == "ins" and start <= item.offset < end]
        ins_by_offset = {item.offset: item for item in ins}
        candidates: list[int] = []
        for item in ins:
            if item.name != "sub" or item.operands != ("r15", "r13"):
                continue
            store = ins_by_offset.get(item.offset + 3)
            if (store and store.name == "store"
                    and store.operands == ("r15", "r0")):
                candidates.append(item.offset)
        expected = lowering_by_proc[proc_index]
        if len(candidates) != len(expected):
            raise ValueError(
                f"{proc[1]} stack push accounting: {len(candidates)} macros "
                f"for {len(expected)} source pushes"
            )
        for push, pc in zip(expected, candidates):
            pcs[by_key[(push.kind, push.node_id, push.ordinal)]] = pc
    if any(pc < 0 for pc in pcs):
        raise ValueError("not every source stack push received an Alpha location")
    return pcs


def u32(value: int) -> bytes:
    return struct.pack("<I", value)


def witness(block_pcs: list[int], transition_pcs: list[int], event_pcs: list[int],
            events: list[Event], access_pcs: list[int],
            accesses: list[LocalAccess], memory_pcs: list[int],
            memory_sites: list[MemorySite], primitive_pcs: list[int],
            primitives: list[ExprPrimitive], push_pcs: list[int],
            pushes: list[StackPush], helper_pc: int, proc_count: int,
            guarded_count: int) -> bytes:
    counts = {kind: sum(event.kind == kind for event in events)
              for kind in range(EVENT_CALL, EVENT_RETURN + 1)}
    return b"".join([
        u32(MAGIC), u32(proc_count), u32(len(block_pcs)),
        u32(len(transition_pcs)), u32(guarded_count),
        u32(len(events)), u32(counts[EVENT_CALL]), u32(counts[EVENT_READ]),
        u32(counts[EVENT_WRITE]), u32(counts[EVENT_EMIT]),
        u32(counts[EVENT_RETURN]),
        u32(sum(len(event.literal) for event in events)),
        u32(len(accesses)),
        u32(sum(access.kind == ACCESS_LOAD for access in accesses)),
        u32(sum(access.kind == ACCESS_STORE for access in accesses)),
        u32(len(memory_sites)),
        u32(sum(site.kind == MEMORY_LOAD for site in memory_sites)),
        u32(sum(site.kind == MEMORY_STORE for site in memory_sites)),
        u32(sum(site.width == 1 for site in memory_sites)),
        u32(sum(site.width == 8 for site in memory_sites)),
        u32(len(primitives)),
        u32(sum(p.kind == PRIMITIVE_LITERAL for p in primitives)),
        u32(sum(p.kind == PRIMITIVE_ARITHMETIC for p in primitives)),
        u32(sum(p.kind == PRIMITIVE_COMPARISON for p in primitives)),
        u32(len(pushes)),
        u32(sum(p.kind == PUSH_BINARY_LEFT for p in pushes)),
        u32(sum(p.kind == PUSH_CALL_ARGUMENT for p in pushes)),
        u32(sum(p.kind == PUSH_STORE_ADDRESS for p in pushes)),
        *(u32(pc) for pc in block_pcs),
        *(u32(pc) for pc in transition_pcs),
        *(u32(pc) for pc in event_pcs),
        *(u32(pc) for pc in access_pcs),
        *(u32(pc) for pc in memory_pcs),
        *(u32(pc) for pc in primitive_pcs),
        *(u32(pc) for pc in push_pcs),
        u32(helper_pc),
    ])


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", type=Path, required=True)
    ap.add_argument("--source", type=Path, required=True)
    ap.add_argument("--assembly", type=Path, required=True)
    ap.add_argument("--tape", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()

    source = args.source.read_bytes()
    tape = bytearray(args.tape.read_bytes())
    blocks = source_blocks(args.repo, source)
    (ast, events, lowering_by_proc, accesses, access_lowering_by_proc,
     memory_sites, memory_lowering_by_proc, primitives,
     primitive_lowering_by_proc, pushes,
     push_lowering_by_proc) = source_events(args.repo, source)
    (items, labels, block_pcs, transition_pcs, jump_pcs,
     target_indices, guarded_count) = locate(
        blocks, args.assembly.read_text(encoding="ascii")
    )
    event_pcs, helper_pc = locate_events(
        ast, events, lowering_by_proc, items, labels, len(tape)
    )
    access_pcs = locate_local_accesses(
        ast, accesses, access_lowering_by_proc, items, labels, len(tape)
    )
    memory_pcs = locate_memory_sites(
        ast, memory_sites, memory_lowering_by_proc, items, labels, len(tape),
        access_pcs,
    )
    primitive_pcs = locate_expr_primitives(
        ast, primitives, primitive_lowering_by_proc, items, labels, len(tape)
    )
    push_pcs = locate_stack_pushes(
        ast, pushes, push_lowering_by_proc, items, labels, len(tape)
    )
    proc_count = len({block.proc_index for block in blocks})
    canonical = witness(
        block_pcs, transition_pcs, event_pcs, events, access_pcs, accesses,
        memory_pcs, memory_sites, primitive_pcs, primitives, push_pcs, pushes,
        helper_pc,
        proc_count, guarded_count,
    )
    args.output.write_bytes(canonical)


if __name__ == "__main__":
    main()
