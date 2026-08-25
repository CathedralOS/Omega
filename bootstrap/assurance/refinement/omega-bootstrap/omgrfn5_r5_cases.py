#!/usr/bin/env python3
"""Focused OMGRFN5 responsibility-5 fixtures, mutations, and observations."""

from __future__ import annotations

import os
import signal
import struct
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "omega-bootstrap" / "gates"))
import checked_ir_v4_reference as ir4

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
    require(head[:2] == (b"OMGRFN5\0", 5), "bad frame")
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
    return FRAME.pack(b"OMGRFN5\0", 5, flags, len(comp), len(witness),
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
    raw = HEADER.pack(b"OMGCKIR\0", 4, 0, 1, 1, 0,
                      HEADER.size + len(payload), *header_counts) + payload
    output.joinpath(name + ".ckir4").write_bytes(raw)
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
        raw=encode(name,output,tables,count,count-1); ir4.decode(raw)

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
        raw=encode(name,output,tables,7,0); ir4.decode(raw)

    chain("frames-64",64); chain("frames-65",65)
    loop("entries-65536",65533); loop("entries-65537",65534)


def constructor_cases(frame: Path, fixtures: Path, output: Path) -> None:
    """Install independently authored CKIR4 rows into one opaque R5 carrier."""
    output.mkdir(parents=True, exist_ok=False)
    parts = split(frame.read_bytes())
    for source in fixtures.glob("*.ckir4"):
        output.joinpath(source.stem + ".rfn").write_bytes(
            pack(parts, ckir=source.read_bytes())
        )


def constructor_resources(output: Path) -> None:
    """Create the first constructor-object frame overflow after full CKIR validation."""
    output.mkdir(parents=True, exist_ok=False)
    count = 10_921
    types = [(0,2,0,0,0,0,0,100), (1,4,0,0,0,0,0,0), (2,4,0,0,1,0,0,0)]
    records = [(0,1,0,0,0), (1,2,0,4,1)]
    fields = [(i,1,i,0) for i in range(4)]
    machines = [(0,0,2,0,0,0,0,0,0,1,0)]
    blocks = [(0,0,2,0,0,0,0,0,count+5,0)]
    operations = [
        (i,0,0,1,1,0,i,0,0,0,i+1,0) for i in range(4)
    ]
    operands = []
    for ordinal in range(count):
        start = len(operands); operands.extend((0,1,2,3))
        operations.append((len(operations),0,0,13,1,0,4+ordinal,2,start,4,0,0))
    operations.append((len(operations),0,0,1,1,0,4+count,0,len(operands),0,70,0))
    terms = [(0,0,0,4,0,0,4+count,NO,len(operands),0,NO,len(operands),0)]
    tables=(types,records,fields,machines,[],blocks,[],[],[],operations,
            [(item,) for item in operands],terms)
    raw=encode("constructor-frame-next",output,tables,count+5,0)
    # The independently maintained decoder must agree this is semantically
    # valid; only derived frame storage is beyond the selected live bound.
    ir4.decode(raw)


def elf_cases(frame: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    parts=split(frame.read_bytes()); elf=parts[6]
    require(elf[:4] == b"\x7fELF" and len(elf) >= 4097, "ELF representative")
    phnum=struct.unpack_from("<H",elf,56)[0]
    require(phnum in (2,3),"ELF segment representative")
    rx=struct.unpack_from("<Q",elf,64+32)[0]
    text=elf[4096:rx]
    lea=text.index(b"\x4c\x8d\x95")
    publish=text.index(b"\x4c\x89\x95",lea)
    wide_store=text.index(b"\x41\x89\x82",lea)
    byte_store=text.index(b"\x41\x88\x82",lea)
    nested_load=text.index(b"\x41\x8b\x83",lea)
    sites={
        "elf-header":24,
        "segment-field":64+56+4,
        "constructor-lea":4096+lea,
        "constructor-object-displacement":4096+lea+3,
        "constructor-publish":4096+publish,
        "constructor-value-displacement":4096+publish+3,
        "constructor-wide-store":4096+wide_store,
        "constructor-field-displacement":4096+wide_store+3,
        "constructor-byte-store":4096+byte_store,
        "constructor-nested-copy":4096+nested_load,
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
    rows=[(float(sec),label) for sec,label in
          (line.split("\t",1) for line in path.read_text().splitlines())]
    setup={"cargo-build","exact-builder","exact-resolver","exact-frame",
           "exact-lowerer","exact-backend","exact-reference","exact-pack",
           "fixture-emit","inherited-resources"}
    def is_build(label: str) -> bool:
        return label.startswith("beta-") or label.startswith("compile-") or label in setup
    build=sum(seconds for seconds,label in rows if is_build(label))
    matrix=sum(seconds for seconds,label in rows if not is_build(label))
    slow=sorted(rows,reverse=True)[:4]
    print("OMGRFN5 responsibility 5 timings: "
          f"build={build:.3f}s matrix={matrix:.3f}s command-sum={build+matrix:.3f}s "
          f"commands={len(rows)} slowest="+
          ",".join(f"{label}:{seconds:.3f}s" for seconds,label in slow))


def main(args):
    if len(args)==3 and args[0]=="phase-cases": phase_cases(Path(args[1]),Path(args[2])); return
    if len(args)==2 and args[0]=="evaluator-cases": evaluator_cases(Path(args[1])); return
    if len(args)==4 and args[0]=="constructor-cases": constructor_cases(Path(args[1]),Path(args[2]),Path(args[3])); return
    if len(args)==2 and args[0]=="constructor-resources": constructor_resources(Path(args[1])); return
    if len(args)==3 and args[0]=="elf-cases": elf_cases(Path(args[1]),Path(args[2])); return
    if args and args[0]=="observe" and args[7]=="--": observe(args[1:]); return
    if len(args)==2 and args[0]=="report": report(Path(args[1])); return
    raise ValueError("bad arguments")


if __name__ == "__main__":
    try: main(sys.argv[1:])
    except (OSError,ValueError,StopIteration) as error:
        print(f"OMGRFN5 responsibility 5 cases: {error}",file=sys.stderr); raise SystemExit(2)
