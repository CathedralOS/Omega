#!/usr/bin/env python3
"""Focused OMGRFN4 responsibility-5 fixtures, mutations, and timed observations."""

from __future__ import annotations

import os
import signal
import struct
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "omega-bootstrap" / "gates"))
import checked_ir_v3_reference as ir3

FRAME = struct.Struct("<8s8I")
HEADER = struct.Struct("<8sHHHH16I")
U32 = struct.Struct("<I")
NO = 0xFFFF_FFFF
ROWS = (
    struct.Struct("<IBBHIIII"), struct.Struct("<IIIIB3x"),
    struct.Struct("<IIII"), struct.Struct("<IIBBHIIIIII"),
    struct.Struct("<IIIII"), struct.Struct("<IIBBHIIIII"),
    struct.Struct("<IIIII"), struct.Struct("<IIIIII"),
    struct.Struct("<I"), struct.Struct("<IIIBBHIIIIII"),
    struct.Struct("<I"), struct.Struct("<IIIBBHIIIIIII"),
)
NAMES = ("types", "records", "fields", "machines", "mparams", "blocks",
         "bparams", "constants", "children", "operations", "operands", "terms")


def require(value: bool, message: str) -> None:
    if not value:
        raise ValueError(message)


def split(raw: bytes):
    require(len(raw) >= FRAME.size, "truncated frame")
    head = FRAME.unpack_from(raw)
    require(head[:2] == (b"OMGRFN4\0", 4), "bad frame")
    _, _, flags, cn, wn, kn, en, result, projection = head
    at = FRAME.size
    parts = []
    for size in (cn, wn, kn, en):
        parts.append(raw[at:at + size]); at += size
    require(at == len(raw), "frame EOF")
    return flags, result, projection, *parts


def pack(parts, *, ckir=None, elf=None, result=None):
    flags, old_result, _, comp, witness, old_ckir, old_elf = parts
    ckir = old_ckir if ckir is None else ckir
    elf = old_elf if elf is None else elf
    result = old_result if result is None else result
    return FRAME.pack(b"OMGRFN4\0", 4, flags, len(comp), len(witness),
                      len(ckir), len(elf), result, result & 255) + comp + witness + ckir + elf


def metadata(raw: bytes):
    fields = HEADER.unpack_from(raw)
    counts = dict(zip(
        ("types", "records", "fields", "machines", "mparams", "blocks",
         "bparams", "operations", "operands", "terms", "values", "places",
         "constants", "children"), fields[7:]))
    bases = {}; at = HEADER.size
    for name, row in zip(NAMES, ROWS):
        bases[name] = at; at += counts[name] * row.size
    require(at == len(raw), "CKIR extent")
    return counts, bases


def phase_cases(frame: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    raw = frame.read_bytes(); parts = split(raw); ckir = parts[5]
    counts, bases = metadata(ckir)
    nodes = [ROWS[7].unpack_from(ckir, bases["constants"] + i * 24)
             for i in range(counts["constants"])]
    children = [U32.unpack_from(ckir, bases["children"] + i * 4)[0]
                for i in range(counts["children"])]

    def changed(name: str, at: int, value: int) -> None:
        altered = bytearray(ckir); U32.pack_into(altered, at, value)
        output.joinpath(name + ".rfn").write_bytes(pack(parts, ckir=bytes(altered)))

    # Keep DAG order/type valid and disconnect node 2 by redirecting the first
    # child of node 16 to same-typed node 0. Only whole-graph reachability fails.
    require(nodes[16][2] < len(children) and children[nodes[16][2]] == 2,
            "unreachable-node representative")
    changed("unreachable-node", bases["children"] + nodes[16][2] * 4, 0)

    op11 = next(bases["operations"] + i * 40 for i in range(counts["operations"])
                if ckir[bases["operations"] + i * 40 + 12] == 11)
    op12 = next(bases["operations"] + i * 40 for i in range(counts["operations"])
                if ckir[bases["operations"] + i * 40 + 12] == 12)
    changed("root-id", op11 + 32, nodes[-1][0] - 1)
    changed("root-imm1", op11 + 36, 1)
    changed("root-operand", bases["operands"] + U32.unpack_from(ckir, op11 + 24)[0] * 4, NO)
    altered = bytearray(ckir); altered[op11 + 13] = 1
    output.joinpath("root-result-shape.rfn").write_bytes(pack(parts, ckir=bytes(altered)))
    altered = bytearray(ckir); altered[op12 + 12] = 9
    output.joinpath("setbe-opcode.rfn").write_bytes(pack(parts, ckir=bytes(altered)))
    output.joinpath("wrong-result.rfn").write_bytes(pack(parts, result=(parts[1] + 1) & NO))


def encode(name: str, output: Path, tables, values: int, places: int) -> bytes:
    payload = b"".join(row.pack(*item) for table, row in zip(tables, ROWS) for item in table)
    counts = tuple(len(table) for table in tables)
    header_counts = (*counts[:7], counts[9], counts[10], counts[11], values,
                     places, counts[7], counts[8])
    raw = HEADER.pack(b"OMGCKIR\0", 3, 0, 1, 1, 0,
                      HEADER.size + len(payload), *header_counts) + payload
    output.joinpath(name + ".ckir3").write_bytes(raw)
    return raw


def evaluator_cases(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    types = [(0,4,0,0,0,0,0,0),(1,3,0,0,0,0,0,1),
             (2,2,0,0,0,0,0,0x7FFFFFFF),(3,1,0,0,0,0,0,255)]
    records = [(0,0,0,0,0)]

    def chain(name: str, count: int):
        machines=[]; blocks=[]; operations=[]; operands=[]; terms=[]
        for machine in range(count):
            start=len(operations)
            if machine+1<count:
                operations += [(len(operations),machine,machine,2,2,0,machine,0,len(operands),0,0,0),
                               (len(operations)+1,machine,machine,10,1,0,machine,3,len(operands),1,machine+1,0)]
                operands.append(machine)
            else:
                operations.append((len(operations),machine,machine,1,1,0,machine,3,len(operands),0,70,0))
            machines.append((machine,0,2,0,0,3,0,0,machine,1,machine))
            blocks.append((machine,machine,2,0,0,0,0,start,len(operations)-start,machine))
            terms.append((machine,machine,machine,4,0,0,machine,NO,0,0,NO,0,0))
        for i,row in enumerate(terms): terms[i]=row[:8]+(len(operands),0,NO,len(operands),0)
        tables=(types,records,[],machines,[],blocks,[],[],[],operations,
                [(x,) for x in operands],terms)
        raw=encode(name,output,tables,count,count-1); ir3.decode(raw)

    def loop(name: str, limit: int):
        local_types=[types[0],(1,2,0,0,0,0,0,0x7FFFFFFF),
                     (2,3,0,0,0,0,0,1),types[3]]
        machines=[(0,0,2,0,0,3,0,0,0,3,0)]
        blocks=[(0,0,2,0,0,0,0,0,1,0),(1,0,2,0,0,0,1,1,4,1),(2,0,2,0,0,1,0,5,1,2)]
        bparams=[(0,1,0,1,0)]
        operations=[(0,0,0,1,1,0,1,1,0,0,0,0),(1,0,1,1,1,0,2,1,0,0,limit,0),
                    (2,0,1,1,1,0,3,1,0,0,1,0),(3,0,1,9,1,0,4,2,0,2,0,0),
                    (4,0,1,8,1,0,5,1,2,2,0,0),(5,0,2,1,1,0,6,3,4,0,70,0)]
        operands=[0,2,0,3,1,5]
        terms=[(0,0,0,1,0,0,NO,1,4,1,NO,5,0),(1,0,1,2,0,0,4,1,5,1,2,6,0),
               (2,0,2,4,0,0,6,NO,6,0,NO,6,0)]
        tables=(local_types,records,[],machines,[],blocks,bparams,[],[],operations,
                [(x,) for x in operands],terms)
        raw=encode(name,output,tables,7,0); ir3.decode(raw)

    chain("frames-64",64); chain("frames-65",65)
    loop("entries-65536",65533); loop("entries-65537",65534)


def elf_cases(frame: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    parts=split(frame.read_bytes()); elf=parts[6]
    require(elf[:4] == b"\x7fELF" and len(elf) >= 4097, "ELF representative")
    phnum=struct.unpack_from("<H",elf,56)[0]
    require(phnum==3,"three-segment representative")
    rx=struct.unpack_from("<Q",elf,64+32)[0]
    text=elf[4096:rx]
    sites={
        "elf-header":24,
        "segment-field":64+56+4,
        "image-byte":rx,
        "rip-constant-displacement":4096+text.index(b"\x48\x8d\x35")+3,
        "setbe-byte":4096+text.index(b"\x0f\x96\xc0")+1,
    }
    for name,at in sites.items():
        changed=bytearray(elf); changed[at]^=1
        output.joinpath(name+".rfn").write_bytes(pack(parts,elf=bytes(changed)))
    output.joinpath("truncated.rfn").write_bytes(pack(parts,elf=elf[:-1]))
    output.joinpath("trailing.rfn").write_bytes(pack(parts,elf=elf+b"\0"))


def observe(args) -> None:
    timeout=float(args[0]); source,output,expected,timings,label=args[1:6]; command=args[7:]
    started=time.monotonic()
    with open(source,"rb") if source!="-" else open("/dev/null","rb") as inp:
        process=subprocess.Popen(command,stdin=inp,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
        try: stdout,stderr=process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid,signal.SIGKILL); process.communicate(); raise ValueError(label+" timed out")
    Path(output).write_bytes(stdout) if output!="-" else None
    with open(timings,"a",encoding="ascii") as out: out.write(f"{time.monotonic()-started:.6f}\t{label}\n")
    require(process.returncode==int(expected),f"{label}: {process.returncode} != {expected}: {stderr[-1000:]!r}")
    if int(expected) and output!="-": require(not stdout,label+" published bytes")


def report(path: Path) -> None:
    rows=[line.split("\t",1) for line in path.read_text().splitlines()]
    print("OMGRFN4 responsibility 5 timings: "+" ".join(f"{label}={float(sec):.3f}s" for sec,label in rows))


def main(args):
    if len(args)==3 and args[0]=="phase-cases": phase_cases(Path(args[1]),Path(args[2])); return
    if len(args)==2 and args[0]=="evaluator-cases": evaluator_cases(Path(args[1])); return
    if len(args)==3 and args[0]=="elf-cases": elf_cases(Path(args[1]),Path(args[2])); return
    if args and args[0]=="observe" and args[7]=="--": observe(args[1:]); return
    if len(args)==2 and args[0]=="report": report(Path(args[1])); return
    raise ValueError("bad arguments")


if __name__ == "__main__":
    try: main(sys.argv[1:])
    except (OSError,ValueError,StopIteration) as error:
        print(f"OMGRFN4 responsibility 5 cases: {error}",file=sys.stderr); raise SystemExit(2)
