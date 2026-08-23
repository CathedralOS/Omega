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
MAGIC = 0x32544342  # little-endian "BCT2"

EVENT_CALL = 1
EVENT_READ = 2
EVENT_WRITE = 3
EVENT_EMIT = 4
EVENT_RETURN = 5


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


def load_parser(repo: Path):
    path = repo / "bootstrap/rungs/beta/reference/beta_parser.py"
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


def source_events(repo: Path, source: bytes) -> tuple[list, list[Event], dict[int, list[Event]]]:
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
    block_index = 0

    def add(kind: int, name: str, literal: bytes, node, block: int,
            arity: int = 0) -> None:
        event = Event(kind, name, literal, id(node), block, arity)
        lexical.append(event)
        event_by_node[id(node)] = event

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
        lowering: list[Event] = []
        entry_block = block_index
        block_index += 1
        for stmt in proc[3]:
            if stmt[0] == "state":
                state_block = block_index
                block_index += 1
                for inner in stmt[2]:
                    lex_stmt(inner, state_block)
                    lower_stmt(inner, lowering)
            else:
                lex_stmt(stmt, entry_block)
                lower_stmt(stmt, lowering)
        lowering_by_proc[proc_index] = lowering
    return ast, lexical, lowering_by_proc


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


def u32(value: int) -> bytes:
    return struct.pack("<I", value)


def witness(block_pcs: list[int], transition_pcs: list[int], event_pcs: list[int],
            events: list[Event], helper_pc: int, proc_count: int,
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
        *(u32(pc) for pc in block_pcs),
        *(u32(pc) for pc in transition_pcs),
        *(u32(pc) for pc in event_pcs),
        u32(helper_pc),
    ])


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", type=Path, required=True)
    ap.add_argument("--source", type=Path, required=True)
    ap.add_argument("--assembly", type=Path, required=True)
    ap.add_argument("--tape", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--retarget-patch-output", type=Path)
    ap.add_argument("--operand-witness-output", type=Path)
    ap.add_argument("--duplicate-witness-output", type=Path)
    ap.add_argument("--missing-witness-output", type=Path)
    ap.add_argument("--noncanonical-witness-output", type=Path)
    ap.add_argument("--call-retarget-patch-output", type=Path)
    ap.add_argument("--read-register-patch-output", type=Path)
    ap.add_argument("--write-register-patch-output", type=Path)
    ap.add_argument("--helper-write-patch-output", type=Path)
    ap.add_argument("--emit-byte-patch-output", type=Path)
    ap.add_argument("--emit-length-patch-output", type=Path)
    ap.add_argument("--emit-pointer-patch-output", type=Path)
    ap.add_argument("--emit-helper-patch-output", type=Path)
    ap.add_argument("--orphan-io-patch-output", type=Path)
    ap.add_argument("--duplicate-event-witness-output", type=Path)
    ap.add_argument("--noncanonical-event-witness-output", type=Path)
    ap.add_argument("--frame-size-patch-output", type=Path)
    ap.add_argument("--saved-fp-patch-output", type=Path)
    ap.add_argument("--frame-base-patch-output", type=Path)
    ap.add_argument("--param-offset-patch-output", type=Path)
    ap.add_argument("--param-register-patch-output", type=Path)
    ap.add_argument("--call-pop-order-patch-output", type=Path)
    ap.add_argument("--call-pop-step-patch-output", type=Path)
    args = ap.parse_args()

    source = args.source.read_bytes()
    tape = bytearray(args.tape.read_bytes())
    blocks = source_blocks(args.repo, source)
    ast, events, lowering_by_proc = source_events(args.repo, source)
    (items, labels, block_pcs, transition_pcs, jump_pcs,
     target_indices, guarded_count) = locate(
        blocks, args.assembly.read_text(encoding="ascii")
    )
    event_pcs, helper_pc = locate_events(
        ast, events, lowering_by_proc, items, labels, len(tape)
    )
    proc_count = len({block.proc_index for block in blocks})
    canonical = witness(
        block_pcs, transition_pcs, event_pcs, events, helper_pc,
        proc_count, guarded_count,
    )
    args.output.write_bytes(canonical)

    if args.retarget_patch_output:
        flat = [(block, guarded) for block in blocks
                for _target_name, guarded in block.transitions]
        transition_index = next(i for i, (_block, guarded) in enumerate(flat)
                                if not guarded)
        source_block = flat[transition_index][0]
        expected_target = target_indices[transition_index]
        alternate = next(
            i for i, block in enumerate(blocks)
            if block.proc_index == source_block.proc_index
            and i != expected_target and "__" in block.label
        )
        jump_pc = jump_pcs[transition_index]
        args.retarget_patch_output.write_bytes(
            struct.pack("<IQ", jump_pc + 1, block_pcs[alternate])
        )

    if args.operand_witness_output:
        lengths = {
            0: 2, 1: 10, **{opcode: 3 for opcode in range(2, 12)},
            12: 9, 13: 10, 14: 10, 15: 11, 16: 11,
            17: 2, 18: 2, 19: 9, 20: 1,
        }
        changed = list(block_pcs)
        for index, pc in enumerate(block_pcs):
            for delta in range(1, lengths[tape[pc]]):
                interior = pc + delta
                if tape[interior] <= 20 and (
                    index + 1 == len(block_pcs) or interior < block_pcs[index + 1]
                ):
                    changed[index] = interior
                    break
            if changed[index] != pc:
                break
        else:
            raise ValueError("no opcode-looking operand byte found for mutation")
        args.operand_witness_output.write_bytes(
            witness(changed, transition_pcs, event_pcs, events, helper_pc,
                    proc_count, guarded_count)
        )

    if args.duplicate_witness_output:
        changed = list(block_pcs)
        changed[1] = changed[0]
        args.duplicate_witness_output.write_bytes(
            witness(changed, transition_pcs, event_pcs, events, helper_pc,
                    proc_count, guarded_count)
        )

    if args.missing_witness_output:
        args.missing_witness_output.write_bytes(canonical[:-4])

    if args.noncanonical_witness_output:
        flat_meta = []
        transition_index = 0
        for block_index, block in enumerate(blocks):
            for _target, guarded in block.transitions:
                flat_meta.append(
                    (block_index, target_indices[transition_index], guarded)
                )
                transition_index += 1
        pair_index = next(
            i for i in range(len(flat_meta) - 1)
            if flat_meta[i][0] == flat_meta[i + 1][0]
            and flat_meta[i][1:] != flat_meta[i + 1][1:]
        )
        changed = list(transition_pcs)
        changed[pair_index], changed[pair_index + 1] = (
            changed[pair_index + 1], changed[pair_index]
        )
        args.noncanonical_witness_output.write_bytes(
            witness(block_pcs, changed, event_pcs, events, helper_pc,
                    proc_count, guarded_count)
        )

    def patch(path: Path | None, offset: int, payload: bytes) -> None:
        if path:
            path.write_bytes(struct.pack("<I", offset) + payload)

    ordinary_index = next(i for i, event in enumerate(events)
                          if event.kind == EVENT_CALL and event.name != "main")
    ordinary = events[ordinary_index]
    alternate_proc = next(proc for proc in ast if proc[1] != ordinary.name)
    patch(args.call_retarget_patch_output, event_pcs[ordinary_index] + 1,
          struct.pack("<Q", labels[alternate_proc[1]]))

    read_index = next(i for i, event in enumerate(events) if event.kind == EVENT_READ)
    write_index = next(i for i, event in enumerate(events) if event.kind == EVENT_WRITE)
    patch(args.read_register_patch_output, event_pcs[read_index] + 1, b"\x01")
    patch(args.write_register_patch_output, event_pcs[write_index] + 1, b"\x01")
    patch(args.orphan_io_patch_output, event_pcs[read_index], bytes([OPS["write"][0]]))

    helper_write = helper_pc + 24
    patch(args.helper_write_patch_output, helper_write + 1, b"\x00")

    emit_index = next(i for i, event in enumerate(events)
                      if event.kind == EVENT_EMIT and event.literal)
    emit_pc = event_pcs[emit_index]
    literal_addr = struct.unpack_from("<Q", tape, emit_pc - 18)[0]
    literal_len = struct.unpack_from("<Q", tape, emit_pc - 8)[0]
    if literal_len != len(events[emit_index].literal):
        raise ValueError("emit literal length does not match its source event")
    patch(args.emit_byte_patch_output, literal_addr,
          bytes([tape[literal_addr] ^ 1]))
    patch(args.emit_length_patch_output, emit_pc - 8,
          struct.pack("<Q", literal_len + 1))
    patch(args.emit_pointer_patch_output, emit_pc - 18,
          struct.pack("<Q", literal_addr + 1))
    patch(args.emit_helper_patch_output, emit_pc + 1,
          struct.pack("<Q", labels[alternate_proc[1]]))

    if args.duplicate_event_witness_output:
        changed = list(event_pcs)
        changed[1] = changed[0]
        args.duplicate_event_witness_output.write_bytes(
            witness(block_pcs, transition_pcs, changed, events, helper_pc,
                    proc_count, guarded_count)
        )
    if args.noncanonical_event_witness_output:
        changed = list(event_pcs)
        pair = next(i for i in range(len(events) - 1)
                    if events[i].kind != events[i + 1].kind)
        changed[pair], changed[pair + 1] = changed[pair + 1], changed[pair]
        args.noncanonical_event_witness_output.write_bytes(
            witness(block_pcs, transition_pcs, changed, events, helper_pc,
                    proc_count, guarded_count)
        )

    def lets_in(statements) -> int:
        return sum(
            1 if stmt[0] == "let" else (
                lets_in(stmt[2]) if stmt[0] == "state" else 0
            )
            for stmt in statements
        )

    frame_proc = next(proc for proc in ast
                      if len(proc[2]) + lets_in(proc[3]) > 0)
    frame_slots = len(frame_proc[2]) + lets_in(frame_proc[3])
    frame_pc = labels[frame_proc[1]]
    patch(args.frame_size_patch_output, frame_pc + 21,
          struct.pack("<Q", frame_slots * 8 + 8))
    patch(args.saved_fp_patch_output, frame_pc + 15, b"\x0d")
    patch(args.frame_base_patch_output, frame_pc + 17, b"\x0d")

    two_param_proc = next(proc for proc in ast if len(proc[2]) == 2)
    param_pc = labels[two_param_proc[1]]
    param_slots = len(two_param_proc[2]) + lets_in(two_param_proc[3])
    param_start = param_pc + 19 + (13 if param_slots else 0)
    patch(args.param_offset_patch_output, param_start + 5,
          struct.pack("<Q", 16))
    patch(args.param_register_patch_output, param_start + 18, b"\x01")

    two_arg_index = next(i for i, event in enumerate(events)
                         if event.kind == EVENT_CALL and event.arity == 2)
    pop_start = event_pcs[two_arg_index] - 32
    patch(args.call_pop_order_patch_output, pop_start + 1, b"\x00")
    patch(args.call_pop_step_patch_output, pop_start + 5,
          struct.pack("<Q", 16))


if __name__ == "__main__":
    main()
