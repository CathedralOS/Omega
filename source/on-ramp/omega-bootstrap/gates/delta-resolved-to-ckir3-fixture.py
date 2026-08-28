#!/usr/bin/env python3
"""Build focused CKIR3 inputs and inspect already-produced CKIR3 bytes.

Compiler execution is deliberately outside this fixture until the canonical
lower-rooted Delta compiler artifact is published.
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "compiler"))
import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


PACKAGE = "33" * 32
NO_ID = 0xFFFFFFFF


def build(output: Path, owner: str, machine: str, sources: list[Path]) -> None:
    entries = [bundle.Entry(f"source-{index}.omg", path.read_bytes())
               for index, path in enumerate(sources)]
    packed = bundle.encode(entries)
    manifest = {
        "target": "linux_x86_64",
        "packages": [{
            "key": PACKAGE,
            "sources": [{"label": entry.label, "module": ""}
                        for entry in entries],
        }],
        "aliases": [],
        "root": {"package": PACKAGE, "source": entries[-1].label,
                 "owner": owner, "machine": machine},
    }
    output.write_bytes(compilation.encode_manifest(manifest, packed))


def inspect_ckir(path: Path) -> str:
    raw = path.read_bytes()
    if len(raw) < 80 or raw[:8] != b"OMGCKIR\0":
        raise ValueError("CKIR3 magic/length")
    major, minor, target, flags = struct.unpack_from("<4H", raw, 8)
    if (major, minor, target) != (3, 0, 1) or flags & ~1:
        raise ValueError("CKIR3 fixed header")
    entry, length, *counts = struct.unpack_from("<16I", raw, 16)
    if length != len(raw):
        raise ValueError("CKIR3 exact length")
    (types, records, fields, machines, machine_parameters, blocks,
     block_parameters, operations, operands, terminators, _values, _places,
     constants, children) = counts
    expected = (
        80 + 24 * types + 20 * records + 16 * fields + 36 * machines
        + 20 * machine_parameters + 32 * blocks + 20 * block_parameters
        + 24 * constants + 4 * children + 40 * operations + 4 * operands
        + 44 * terminators
    )
    if expected != len(raw) or terminators != blocks or (
            flags & 1 and entry == NO_ID):
        raise ValueError("CKIR3 table length/relation")
    at = (80 + 24 * types + 20 * records + 16 * fields + 36 * machines
          + 20 * machine_parameters + 32 * blocks + 20 * block_parameters)
    nodes = [struct.unpack_from("<6I", raw, at + 24 * index)
             for index in range(constants)]
    at += 24 * constants
    child_ids = struct.unpack_from(f"<{children}I", raw, at) if children else ()
    at += 4 * children
    opcodes: list[int] = []
    roots: list[int] = []
    for index in range(operations):
        row = raw[at + 40 * index:at + 40 * (index + 1)]
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
    unique = ",".join(map(str, sorted(set(opcodes))))
    return (f"types={types} constants={constants} children={children} "
            f"ops={operations} opcodes={unique} roots={len(roots)}")


def main(args: list[str]) -> int:
    if len(args) >= 5 and args[0] == "build":
        build(Path(args[1]), args[2], args[3],
              [Path(item) for item in args[4:]])
        return 0
    if len(args) == 2 and args[0] == "inspect":
        print(inspect_ckir(Path(args[1])))
        return 0
    raise ValueError("usage: build OUTPUT OWNER MACHINE SOURCE... | inspect CKIR3")


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as error:
        print(f"delta-resolved-to-ckir3-fixture: {error}", file=sys.stderr)
        raise SystemExit(2)
