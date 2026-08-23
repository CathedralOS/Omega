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
MAGIC = 0x31435442  # little-endian "BCT1"


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
    return block_pcs, transition_pcs, transition_jmps, target_indices, guarded_count


def u32(value: int) -> bytes:
    return struct.pack("<I", value)


def witness(block_pcs: list[int], transition_pcs: list[int], proc_count: int,
            guarded_count: int) -> bytes:
    return b"".join([
        u32(MAGIC), u32(proc_count), u32(len(block_pcs)),
        u32(len(transition_pcs)), u32(guarded_count),
        *(u32(pc) for pc in block_pcs),
        *(u32(pc) for pc in transition_pcs),
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
    args = ap.parse_args()

    source = args.source.read_bytes()
    tape = bytearray(args.tape.read_bytes())
    blocks = source_blocks(args.repo, source)
    block_pcs, transition_pcs, jump_pcs, target_indices, guarded_count = locate(
        blocks, args.assembly.read_text(encoding="ascii")
    )
    proc_count = len({block.proc_index for block in blocks})
    canonical = witness(block_pcs, transition_pcs, proc_count, guarded_count)
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
            witness(changed, transition_pcs, proc_count, guarded_count)
        )

    if args.duplicate_witness_output:
        changed = list(block_pcs)
        changed[1] = changed[0]
        args.duplicate_witness_output.write_bytes(
            witness(changed, transition_pcs, proc_count, guarded_count)
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
            witness(block_pcs, changed, proc_count, guarded_count)
        )


if __name__ == "__main__":
    main()
