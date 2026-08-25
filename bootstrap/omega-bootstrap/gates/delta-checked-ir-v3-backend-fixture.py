#!/usr/bin/env python3
"""Independent CKIR3 nested-constant/<= fixture and focused ELF checker."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path


NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH16I")
TYPE = struct.Struct("<IBBHIIII")
RECORD = struct.Struct("<IIIIB3x")
FIELD = struct.Struct("<IIII")
MACHINE = struct.Struct("<IIBBHIIIIII")
PARAM = struct.Struct("<IIIII")
BLOCK = struct.Struct("<IIBBHIIIII")
CONSTANT = struct.Struct("<IIIIII")
WORD = struct.Struct("<I")
OPERATION = struct.Struct("<IIIBBHIIIIII")
TERMINATOR = struct.Struct("<IIIBBHIIIIIII")


def expected() -> bytes:
    types = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 4, 0, 0, 1, 0, 0, 0),
        (2, 2, 0, 0, 0, 0, 0, 0x7FFF_FFFF),
        (3, 3, 0, 0, 0, 0, 0, 1),
        (4, 5, 1, 0, 1, 2, 0, 0),
    ]
    records = [
        (0, 0, 0, 2, 0),
        (1, 1, 2, 2, 1),
    ]
    fields = [
        (0, 0, 0, 4),
        (1, 0, 1, 2),
        (2, 1, 0, 2),
        (3, 1, 1, 2),
    ]
    machines = [(0, 0, 2, 0, 0, 2, 0, 0, 0, 3, 0)]
    blocks = [
        (0, 0, 2, 0, 0, 0, 0, 0, 9, 0),
        (1, 0, 2, 0, 0, 0, 0, 9, 1, 1),
        (2, 0, 2, 0, 0, 0, 0, 10, 1, 2),
    ]
    constants = [
        (0, 2, 0, 0, 10, 0),
        (1, 2, 0, 0, 20, 0),
        (2, 2, 0, 0, 30, 0),
        (3, 2, 0, 0, 40, 0),
        (4, 1, 0, 2, 0, 0),
        (5, 1, 2, 2, 0, 0),
        (6, 4, 4, 2, 0, 0),
    ]
    constant_children = [(0,), (1,), (2,), (3,), (4,), (5,)]
    operations = [
        (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 0, 3, 2, 0, 1, 4, 0, 1, 0, 0),
        (2, 0, 0, 11, 0, 0, NO_ID, NO_ID, 1, 1, 6, 0),
        (3, 0, 0, 1, 1, 0, 0, 2, 2, 0, 1, 0),
        (4, 0, 0, 4, 2, 0, 2, 1, 2, 2, 0, 0),
        (5, 0, 0, 3, 2, 0, 3, 2, 4, 1, 3, 0),
        (6, 0, 0, 5, 1, 0, 1, 2, 5, 1, 0, 0),
        (7, 0, 0, 1, 1, 0, 2, 2, 6, 0, 30, 0),
        (8, 0, 0, 12, 1, 0, 3, 3, 6, 2, 0, 0),
        (9, 0, 1, 1, 1, 0, 4, 2, 8, 0, 70, 0),
        (10, 0, 2, 1, 1, 0, 5, 2, 8, 0, 69, 0),
    ]
    operands = [(0,), (1,), (1,), (0,), (2,), (3,), (2,), (1,)]
    terminators = [
        (0, 0, 0, 2, 0, 0, 3, 1, 8, 0, 2, 8, 0),
        (1, 0, 1, 4, 0, 0, 4, NO_ID, 8, 0, NO_ID, 8, 0),
        (2, 0, 2, 4, 0, 0, 5, NO_ID, 8, 0, NO_ID, 8, 0),
    ]
    tables = (
        (types, TYPE),
        (records, RECORD),
        (fields, FIELD),
        (machines, MACHINE),
        ([], PARAM),
        (blocks, BLOCK),
        ([], PARAM),
        (constants, CONSTANT),
        (constant_children, WORD),
        (operations, OPERATION),
        (operands, WORD),
        (terminators, TERMINATOR),
    )
    payload = b"".join(row_type.pack(*row) for rows, row_type in tables for row in rows)
    counts = {
        "types": len(types), "records": len(records), "fields": len(fields),
        "machines": len(machines), "mparams": 0, "blocks": len(blocks),
        "bparams": 0, "operations": len(operations), "operands": len(operands),
        "terms": len(terminators), "values": 6, "places": 4,
        "constants": len(constants), "children": len(constant_children),
    }
    return HEADER.pack(
        b"OMGCKIR\0", 3, 0, 1, 1, 0, HEADER.size + len(payload),
        counts["types"], counts["records"], counts["fields"], counts["machines"],
        counts["mparams"], counts["blocks"], counts["bparams"],
        counts["operations"], counts["operands"], counts["terms"],
        counts["values"], counts["places"], counts["constants"], counts["children"],
    ) + payload


def no_pool() -> bytes:
    types = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 2, 0, 0, 0, 0, 0, 0x7FFF_FFFF),
    ]
    records = [(0, 0, 0, 0, 0)]
    machines = [(0, 0, 2, 0, 0, 1, 0, 0, 0, 1, 0)]
    blocks = [(0, 0, 2, 0, 0, 0, 0, 0, 1, 0)]
    operations = [(0, 0, 0, 1, 1, 0, 0, 1, 0, 0, 70, 0)]
    terminators = [(0, 0, 0, 4, 0, 0, 0, NO_ID, 0, 0, NO_ID, 0, 0)]
    tables = (
        (types, TYPE), (records, RECORD), ([], FIELD), (machines, MACHINE),
        ([], PARAM), (blocks, BLOCK), ([], PARAM), ([], CONSTANT), ([], WORD),
        (operations, OPERATION), ([], WORD), (terminators, TERMINATOR),
    )
    payload = b"".join(row_type.pack(*row) for rows, row_type in tables for row in rows)
    return HEADER.pack(
        b"OMGCKIR\0", 3, 0, 1, 1, 0, HEADER.size + len(payload),
        2, 1, 0, 1, 0, 1, 0, 1, 0, 1, 1, 0, 0, 0,
    ) + payload


def large_image(outer_length: int) -> bytes:
    types = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 2, 0, 0, 0, 0, 0, 0x7FFF_FFFF),
        (2, 5, 1, 0, 1, 1024, 0, 0),
        (3, 5, 1, 0, 2, outer_length, 0, 0),
    ]
    records = [(0, 0, 0, 1, 0)]
    fields = [(0, 0, 0, 3)]
    machines = [(0, 0, 2, 0, 0, 1, 0, 0, 0, 1, 0)]
    blocks = [(0, 0, 2, 0, 0, 0, 0, 0, 4, 0)]
    constants = [
        (0, 1, 0, 0, 1, 0),
        (1, 2, 0, 1024, 0, 0),
        (2, 3, 1024, outer_length, 0, 0),
    ]
    children = [(0,)] * 1024 + [(1,)] * outer_length
    operations = [
        (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 0, 3, 2, 0, 1, 3, 0, 1, 0, 0),
        (2, 0, 0, 11, 0, 0, NO_ID, NO_ID, 1, 1, 2, 0),
        (3, 0, 0, 1, 1, 0, 0, 1, 2, 0, 70, 0),
    ]
    operands = [(0,), (1,)]
    terminators = [(0, 0, 0, 4, 0, 0, 0, NO_ID, 2, 0, NO_ID, 2, 0)]
    tables = (
        (types, TYPE), (records, RECORD), (fields, FIELD), (machines, MACHINE),
        ([], PARAM), (blocks, BLOCK), ([], PARAM), (constants, CONSTANT),
        (children, WORD), (operations, OPERATION), (operands, WORD),
        (terminators, TERMINATOR),
    )
    payload = b"".join(row_type.pack(*row) for rows, row_type in tables for row in rows)
    return HEADER.pack(
        b"OMGCKIR\0", 3, 0, 1, 1, 0, HEADER.size + len(payload),
        4, 1, 1, 1, 0, 1, 0, 4, 2, 1, 1, 2, 3, len(children),
    ) + payload


def bases(raw: bytes) -> dict[str, int]:
    header = HEADER.unpack_from(raw)
    counts = {
        "types": header[7], "records": header[8], "fields": header[9],
        "machines": header[10], "mparams": header[11], "blocks": header[12],
        "bparams": header[13], "operations": header[14], "operands": header[15],
        "terms": header[16], "constants": header[19], "children": header[20],
    }
    order = (
        ("types", TYPE), ("records", RECORD), ("fields", FIELD),
        ("machines", MACHINE), ("mparams", PARAM), ("blocks", BLOCK),
        ("bparams", PARAM), ("constants", CONSTANT), ("children", WORD),
        ("operations", OPERATION), ("operands", WORD), ("terms", TERMINATOR),
    )
    cursor = HEADER.size
    result: dict[str, int] = {}
    for name, row_type in order:
        result[name] = cursor
        cursor += counts[name] * row_type.size
    if cursor != len(raw):
        raise ValueError(f"fixture table extent {cursor} != {len(raw)}")
    return result


def mutate(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    canonical = expected()
    table = bases(canonical)

    def one(name: str, offset: int, value: int, form: str = "I") -> None:
        raw = bytearray(canonical)
        struct.pack_into("<" + form, raw, offset, value)
        out.joinpath(name + ".ckir3").write_bytes(raw)

    one("schema-major", 8, 2, "H")
    one("constant-scalar-type", table["constants"] + 4, 3)
    one("constant-order-duplicate", table["constants"] + CONSTANT.size + 16, 10)
    one("constant-forward-child", table["children"], 6)
    one("constant-unreachable", table["children"] + 5 * 4, 4)
    one("copy-root-type", table["operations"] + 2 * OPERATION.size + 32, 5)
    one("copy-immediate-one", table["operations"] + 2 * OPERATION.size + 36, 1)
    one("less-equal-result-type", table["operations"] + 8 * OPERATION.size + 20, 2)
    one("constant-node-resource", 72, 8193)
    one("constant-child-resource", 76, 16385)
    one("encoded-byte-resource", 20, 2522193)


def check_ckir(path: Path) -> None:
    actual = path.read_bytes()
    wanted = expected()
    if actual != wanted:
        limit = min(len(actual), len(wanted))
        offset = next((i for i in range(limit) if actual[i] != wanted[i]), limit)
        raise ValueError(
            f"CKIR3 mismatch at {offset}: actual={actual[offset:offset+16].hex()} "
            f"expected={wanted[offset:offset+16].hex()} lengths={len(actual)}/{len(wanted)}"
        )
    print("70")


def check_elf(ckir_path: Path, elf_path: Path) -> None:
    check_ckir(ckir_path)
    data = elf_path.read_bytes()
    if len(data) < 4096 or data[:4] != b"\x7fELF":
        raise ValueError("not an ELF64 image")
    ehdr = struct.unpack_from("<16sHHIQQQIHHHHHH", data)
    if ehdr[1:5] != (2, 62, 1, 0x401000) or ehdr[9] != 56 or ehdr[10] != 3:
        raise ValueError(f"unexpected ELF header {ehdr[1:12]}")
    phdrs = [struct.unpack_from("<IIQQQQQQ", data, 64 + i * 56) for i in range(3)]
    rx, ro, rw = phdrs
    rx_size = rx[5]
    if rx != (1, 5, 0, 0x400000, 0x400000, rx_size, rx_size, 4096):
        raise ValueError(f"bad RX segment {rx}")
    if ro != (1, 4, rx_size, 0x400000 + rx_size, 0x400000 + rx_size, 4096, 4096, 4096):
        raise ValueError(f"bad R segment {ro}")
    if rw != (1, 6, rx_size + 4096, 0x400000 + rx_size + 4096, 0x400000 + rx_size + 4096, 0, 4096, 4096):
        raise ValueError(f"bad RW segment {rw}")
    if len(data) != rx_size + 4096:
        raise ValueError(f"ELF length {len(data)} != {rx_size + 4096}")
    pool = struct.pack("<4I", 10, 20, 30, 40)
    if data[rx_size:rx_size + len(pool)] != pool or any(data[rx_size + len(pool):]):
        raise ValueError("derived constant image or zero padding mismatch")
    bss_displacement = struct.unpack_from("<i", data, 4099)[0]
    if 4103 + bss_displacement != rx_size + 4096:
        raise ValueError("entry shim BSS target does not follow R segment")
    marker = b"\x49\x89\xc2\x48\x8d\x35"
    copy_at = data.find(marker, 4096, rx_size)
    if copy_at < 0 or data.find(marker, copy_at + 1, rx_size) >= 0:
        raise ValueError("CopyAggregateConst template count")
    displacement = struct.unpack_from("<i", data, copy_at + 6)[0]
    if copy_at + 10 + displacement != rx_size:
        raise ValueError("CopyAggregateConst constant target")
    if data[copy_at + 10:copy_at + 20] != b"\x4c\x89\xd7\xb9\x10\x00\x00\x00\xf3\xa4":
        raise ValueError("CopyAggregateConst template tail")
    if data.count(b"\x0f\x96\xc0\x0f\xb6\xc0") != 1:
        raise ValueError("LessEqual setbe template count")
    print(f"70 {len(data)} {rx_size} 4096")


def check_no_pool_elf(ckir_path: Path, elf_path: Path) -> None:
    if ckir_path.read_bytes() != no_pool():
        raise ValueError("no-pool CKIR3 mismatch")
    data = elf_path.read_bytes()
    if len(data) != 8192 or data[:4] != b"\x7fELF":
        raise ValueError("no-pool ELF extent or magic")
    ehdr = struct.unpack_from("<16sHHIQQQIHHHHHH", data)
    if ehdr[1:5] != (2, 62, 1, 0x401000) or ehdr[9] != 56 or ehdr[10] != 2:
        raise ValueError("no-pool ELF header")
    rx, rw = [struct.unpack_from("<IIQQQQQQ", data, 64 + i * 56) for i in range(2)]
    if rx != (1, 5, 0, 0x400000, 0x400000, 8192, 8192, 4096):
        raise ValueError(f"no-pool RX segment {rx}")
    if rw != (1, 6, 8192, 0x402000, 0x402000, 0, 4096, 4096):
        raise ValueError(f"no-pool RW segment {rw}")
    bss_displacement = struct.unpack_from("<i", data, 4099)[0]
    if 4103 + bss_displacement != 8192:
        raise ValueError("no-pool entry shim BSS target")
    print("70 8192 2")


def check_image_boundary_elf(ckir_path: Path, elf_path: Path) -> None:
    if ckir_path.read_bytes() != large_image(32):
        raise ValueError("image-boundary CKIR3 mismatch")
    data = elf_path.read_bytes()
    ehdr = struct.unpack_from("<16sHHIQQQIHHHHHH", data)
    if ehdr[10] != 3:
        raise ValueError("image-boundary program-header count")
    rx, ro, rw = [struct.unpack_from("<IIQQQQQQ", data, 64 + i * 56) for i in range(3)]
    rx_size = rx[5]
    if ro[5:7] != (131072, 131072) or rw[6] != 131072:
        raise ValueError("image-boundary R/RW extents")
    if len(data) != rx_size + 131072:
        raise ValueError("image-boundary file extent")
    if data[rx_size:] != b"\x01\x00\x00\x00" * 32768:
        raise ValueError("image-boundary derived pool bytes")
    print(f"70 {len(data)} {rx_size} 131072")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    emit = sub.add_parser("emit")
    emit.add_argument("output", type=Path)
    emit_no_pool = sub.add_parser("emit-no-pool")
    emit_no_pool.add_argument("output", type=Path)
    emit_image_boundary = sub.add_parser("emit-image-boundary")
    emit_image_boundary.add_argument("output", type=Path)
    emit_image_resource = sub.add_parser("emit-image-resource")
    emit_image_resource.add_argument("output", type=Path)
    check = sub.add_parser("check")
    check.add_argument("ckir", type=Path)
    mutations = sub.add_parser("mutations")
    mutations.add_argument("directory", type=Path)
    elf = sub.add_parser("check-elf")
    elf.add_argument("ckir", type=Path)
    elf.add_argument("elf", type=Path)
    no_pool_elf = sub.add_parser("check-no-pool-elf")
    no_pool_elf.add_argument("ckir", type=Path)
    no_pool_elf.add_argument("elf", type=Path)
    image_boundary_elf = sub.add_parser("check-image-boundary-elf")
    image_boundary_elf.add_argument("ckir", type=Path)
    image_boundary_elf.add_argument("elf", type=Path)
    args = parser.parse_args()
    if args.command == "emit":
        args.output.write_bytes(expected())
    elif args.command == "emit-no-pool":
        args.output.write_bytes(no_pool())
    elif args.command == "emit-image-boundary":
        args.output.write_bytes(large_image(32))
    elif args.command == "emit-image-resource":
        args.output.write_bytes(large_image(33))
    elif args.command == "check":
        check_ckir(args.ckir)
    elif args.command == "mutations":
        mutate(args.directory)
    elif args.command == "check-elf":
        check_elf(args.ckir, args.elf)
    elif args.command == "check-no-pool-elf":
        check_no_pool_elf(args.ckir, args.elf)
    else:
        check_image_boundary_elf(args.ckir, args.elf)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, struct.error) as error:
        raise SystemExit(f"CKIR3 backend fixture: {error}")
