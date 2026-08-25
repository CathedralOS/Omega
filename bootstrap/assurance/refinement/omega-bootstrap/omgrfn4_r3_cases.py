#!/usr/bin/env python3
"""Focused OMGRFN4 responsibility-3 mutations and exact CKIR3 census."""

from __future__ import annotations

import os
import signal
import struct
import subprocess
import sys
import time
from pathlib import Path


NO_ID = 0xFFFF_FFFF
FRAME = struct.Struct("<8s8I")
CKIR = struct.Struct("<8sHHHH16I")
U32 = struct.Struct("<I")
ROWS = {
    "types": 24, "records": 20, "fields": 16, "machines": 36,
    "mparams": 20, "blocks": 32, "bparams": 20, "constants": 24,
    "children": 4, "operations": 40, "operands": 4, "terms": 44,
}
ORDER = tuple(ROWS)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def split(raw: bytes) -> tuple[int, int, int, bytes, bytes, bytes, bytes]:
    require(len(raw) >= FRAME.size, "truncated OMGRFN4")
    magic, version, flags, cn, wn, kn, en, result, exit_code = FRAME.unpack_from(raw)
    require((magic, version) == (b"OMGRFN4\0", 4), "OMGRFN4 identity")
    at = FRAME.size
    comp = raw[at:at + cn]; at += cn
    witness = raw[at:at + wn]; at += wn
    ckir = raw[at:at + kn]; at += kn
    elf = raw[at:at + en]; at += en
    require(at == len(raw), "OMGRFN4 exact EOF")
    return flags, result, exit_code, comp, witness, ckir, elf


def pack(parts: tuple[int, int, int, bytes, bytes, bytes, bytes], *,
         ckir: bytes | None = None, elf: bytes | None = None,
         result: int | None = None) -> bytes:
    flags, old_result, _, comp, witness, old_ckir, old_elf = parts
    ckir = old_ckir if ckir is None else ckir
    elf = old_elf if elf is None else elf
    result = old_result if result is None else result
    return FRAME.pack(
        b"OMGRFN4\0", 4, flags, len(comp), len(witness), len(ckir), len(elf),
        result, result & 255,
    ) + comp + witness + ckir + elf


def meta(raw: bytes) -> tuple[dict[str, int], dict[str, int]]:
    fields = CKIR.unpack_from(raw)
    require(fields[:4] == (b"OMGCKIR\0", 3, 0, 1), "CKIR3 identity")
    raw_counts = fields[7:]
    names = (
        "types", "records", "fields", "machines", "mparams", "blocks",
        "bparams", "operations", "operands", "terms", "values", "places",
        "constants", "children",
    )
    counts = dict(zip(names, raw_counts))
    at = CKIR.size
    bases: dict[str, int] = {}
    for name in ORDER:
        bases[name] = at
        at += ROWS[name] * counts[name]
    require(at == len(raw), "CKIR3 exact table extent")
    return counts, bases


def word(raw: bytes, at: int, value: int) -> bytes:
    changed = bytearray(raw)
    U32.pack_into(changed, at, value)
    return bytes(changed)


def node(raw: bytes, base: int, index: int) -> tuple[int, ...]:
    return struct.unpack_from("<6I", raw, base + index * 24)


def type_row(raw: bytes, base: int, index: int) -> tuple[int, ...]:
    return struct.unpack_from("<IBBHIIII", raw, base + index * 24)


def put(out: Path, name: str, raw: bytes) -> None:
    out.joinpath(name + ".rfn").write_bytes(raw)


def cases(frame_path: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    original = frame_path.read_bytes()
    parts = split(original)
    ckir = parts[5]
    counts, bases = meta(ckir)
    require(counts["constants"] >= 3 and counts["children"] >= 1,
            "representative constant graph")

    def ccase(name: str, changed: bytes) -> None:
        require(len(changed) == len(ckir), name + " CKIR extent")
        put(output, name, pack(parts, ckir=changed))

    constants = bases["constants"]
    children = bases["children"]
    nodes = [node(ckir, constants, index) for index in range(counts["constants"])]
    types = [type_row(ckir, bases["types"], index) for index in range(counts["types"])]

    ccase("count-framing", word(ckir, 72, counts["constants"] - 1))
    ccase("dense-id", word(ckir, constants + 24, 0))
    ccase("empty-span-offset", word(ckir, constants + 8, 1))
    ccase("reserved", word(ckir, constants + 20, 1))

    scalar_ids = [index for index, row in enumerate(nodes) if types[row[1]][1] <= 3]
    structural_ids = [index for index, row in enumerate(nodes) if types[row[1]][1] >= 4]
    require(scalar_ids and structural_ids, "scalar/structural representatives")
    scalar = scalar_ids[0]
    scalar_row = nodes[scalar]
    high = types[scalar_row[1]][7]
    ccase("scalar-range", word(ckir, constants + scalar * 24 + 16, high + 1))
    ccase("scalar-type-arity", word(
        ckir, constants + scalar * 24 + 4, nodes[structural_ids[0]][1]))

    parent = next(index for index in structural_ids if nodes[index][3] > 0)
    parent_row = nodes[parent]
    ccase("structural-arity", word(
        ckir, constants + parent * 24 + 12, parent_row[3] - 1))
    ccase("child-back-edge", word(
        ckir, children + parent_row[2] * 4, parent))

    actual_child = U32.unpack_from(ckir, children + parent_row[2] * 4)[0]
    expected_type = nodes[actual_child][1]
    wrong = next((index for index in range(parent)
                  if nodes[index][1] != expected_type), None)
    require(wrong is not None, "wrong-type earlier child representative")
    ccase("child-type-layout", word(
        ckir, children + parent_row[2] * 4, wrong))

    # Reorder one independent lower-height node after a higher-height node,
    # rebuilding dense IDs, spans, and child IDs so back-edge and framing rules
    # remain valid and only recomputed height/key order is noncanonical.
    heights: list[int] = []
    child_lists: list[list[int]] = []
    for index, row in enumerate(nodes):
        values = [U32.unpack_from(ckir, children + (row[2] + offset) * 4)[0]
                  for offset in range(row[3])]
        child_lists.append(values)
        heights.append(0 if types[row[1]][1] <= 3 else
                       1 + max((heights[value] for value in values), default=-1))
    height_pair = None
    for right_height in structural_ids:
        maximum_child = max(child_lists[right_height], default=-1)
        for left_height in range(maximum_child + 1, right_height):
            if heights[left_height] >= heights[right_height]:
                continue
            if any(left_height in child_lists[middle]
                   for middle in range(left_height + 1, right_height + 1)):
                continue
            height_pair = (left_height, right_height)
            break
        if height_pair is not None:
            break
    require(height_pair is not None, "independent height-order representative")
    left_height, right_height = height_pair
    order = list(range(len(nodes)))
    order[left_height], order[right_height] = order[right_height], order[left_height]
    remap = {old: new for new, old in enumerate(order)}
    rebuilt_nodes = bytearray()
    rebuilt_children: list[int] = []
    for new_id, old_id in enumerate(order):
        old = nodes[old_id]
        mapped = [remap[value] for value in child_lists[old_id]]
        require(all(value < new_id for value in mapped),
                "height-order back-edge isolation")
        rebuilt_nodes.extend(struct.pack(
            "<6I", new_id, old[1], len(rebuilt_children), len(mapped), old[4], old[5]))
        rebuilt_children.extend(mapped)
    require(len(rebuilt_nodes) == counts["constants"] * 24 and
            len(rebuilt_children) == counts["children"], "height-order extent")
    changed = bytearray(ckir)
    changed[constants:children] = rebuilt_nodes
    for index, value in enumerate(rebuilt_children):
        U32.pack_into(changed, children + index * 4, value)
    ccase("height-order", bytes(changed))

    pair = next((
        (left, right) for left, right in zip(scalar_ids, scalar_ids[1:])
        if nodes[left][1] == nodes[right][1]
        and nodes[left][4] > types[nodes[left][1]][6]
    ), None)
    require(pair is not None, "same-type scalar ordering representative")
    left, right = pair
    ccase("key-order", word(
        ckir, constants + right * 24 + 16, nodes[left][4] - 1))
    ccase("duplicate-key", word(
        ckir, constants + right * 24 + 16, nodes[left][4]))

    # The declaration/type/layout join is independently owned even when the
    # constant table remains untouched.
    ccase("type-layout-join", word(
        ckir, bases["types"] + scalar_row[1] * 24 + 16,
        types[scalar_row[1]][6] + 1))

    ccase("constant-count-resource", word(ckir, 72, 8193))
    ccase("child-count-resource", word(ckir, 76, 16385))
    ccase("ckir2-inner-version", word(ckir, 8, 2))

    opcode11 = next(
        bases["operations"] + index * 40
        for index in range(counts["operations"])
        if ckir[bases["operations"] + index * 40 + 12] == 11
    )
    ccase("opaque-opcode11-root", word(ckir, opcode11 + 32, NO_ID))

    # A scalar change strictly between its canonical neighbours remains an
    # intrinsically valid graph but no longer corresponds to the source body.
    opaque = next((
        (left, right) for left, right in zip(scalar_ids, scalar_ids[1:])
        if nodes[left][1] == nodes[right][1] and nodes[left][4] + 1 < nodes[right][4]
    ), None)
    require(opaque is not None, "source-opaque scalar gap")
    left, _ = opaque
    ccase("opaque-source-constant", word(
        ckir, constants + left * 24 + 16, nodes[left][4] + 1))
    put(output, "opaque-result", pack(parts, result=(parts[1] + 1) & NO_ID))
    changed_elf = bytearray(parts[6])
    require(bool(changed_elf), "opaque ELF representative")
    changed_elf[0] ^= 1
    put(output, "opaque-elf", pack(parts, elf=bytes(changed_elf)))

    # A validated public extent selects 252 before the inevitably malformed
    # component relation can downgrade it.
    over = bytearray(original)
    U32.pack_into(over, 24, 2_522_193)
    put(output, "declared-ckir-resource", bytes(over))


def summary(frame_path: Path, expected_nodes: int, expected_children: int) -> None:
    counts, _ = meta(split(frame_path.read_bytes())[5])
    require((counts["constants"], counts["children"]) ==
            (expected_nodes, expected_children), "constant census")
    print(f"nodes={expected_nodes} children={expected_children}")


def observe(arguments: list[str]) -> None:
    require(len(arguments) >= 8 and arguments[6] == "--", "observe arguments")
    timeout = float(arguments[0])
    input_name, output_name = arguments[1:3]
    expected = int(arguments[3])
    timing_name, label = arguments[4:6]
    command = arguments[7:]
    started = time.monotonic()
    try:
        with (open(input_name, "rb") if input_name != "-" else open("/dev/null", "rb")) as source:
            with (open(output_name, "wb") if output_name != "-" else open("/dev/null", "wb")) as output:
                process = subprocess.Popen(
                    command, stdin=source, stdout=output, stderr=subprocess.PIPE,
                    start_new_session=True,
                )
                try:
                    _, stderr = process.communicate(timeout=timeout)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.communicate()
                    raise
    except subprocess.TimeoutExpired as error:
        raise ValueError(f"{label} exceeded {timeout:.0f}s") from error
    elapsed = time.monotonic() - started
    with open(timing_name, "a", encoding="ascii") as timings:
        timings.write(f"{elapsed:.6f}\t{label}\n")
    if process.returncode != expected:
        detail = stderr.decode("utf-8", errors="replace")[-1200:]
        raise ValueError(
            f"{label} status {process.returncode}, expected {expected}: {detail}"
        )
    if expected and output_name != "-" and Path(output_name).stat().st_size:
        raise ValueError(f"{label} published bytes on rejection")


def report(path: Path) -> None:
    rows = []
    for line in path.read_text(encoding="ascii").splitlines():
        elapsed, label = line.split("\t", 1)
        rows.append((label, float(elapsed)))
    wanted = {
        "beta-build", "compile-resolver", "compile-lowerer",
        "unicode-resolver", "unicode-lowerer", "compact-resolver",
        "compact-lowerer", "unicode-check", "compact-check", "controls",
    }
    print("OMGRFN4 responsibility 3 timings: " + " ".join(
        f"{label}={elapsed:.3f}s" for label, elapsed in rows if label in wanted
    ))


def main(arguments: list[str]) -> None:
    if len(arguments) == 3 and arguments[0] == "cases":
        cases(Path(arguments[1]), Path(arguments[2])); return
    if len(arguments) == 4 and arguments[0] == "summary":
        summary(Path(arguments[1]), int(arguments[2]), int(arguments[3])); return
    if arguments and arguments[0] == "observe":
        observe(arguments[1:]); return
    if len(arguments) == 2 and arguments[0] == "report":
        report(Path(arguments[1])); return
    raise ValueError(
        "usage: cases OMGRFN4 OUT | summary OMGRFN4 NODES CHILDREN | "
        "observe TIME IN OUT STATUS TIMINGS LABEL -- COMMAND... | report TIMINGS"
    )


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except (OSError, ValueError, StopIteration) as error:
        print(f"OMGRFN4 responsibility 3 cases: {error}", file=sys.stderr)
        raise SystemExit(2)
