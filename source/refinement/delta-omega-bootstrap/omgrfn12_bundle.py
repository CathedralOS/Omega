#!/usr/bin/env python3
"""Untrusted exact-byte packer for the private OMGRFN12 refinement frame."""
from __future__ import annotations
import argparse, struct, sys
from pathlib import Path
from omgrfn6_bundle import HEADER, MAX_CKIR, MAX_ELF, MAX_FRAME, MAX_OMGCOMP, MAX_WITNESS, NO_RESULT
MAGIC=b"OMGRFNC\0"; CKIR_MAGIC=b"OMGCKIR\0"
WITNESS_MAJORS={b"OMGRSW1\0":1,b"OMGRSW2\0":2,b"OMGRSW3\0":3}
def bounded(path:Path,ceiling:int,label:str)->bytes:
    contents=path.read_bytes()
    if len(contents)>ceiling: raise SystemExit(f"{label} exceeds OMGRFN12 ceiling")
    return contents
def require_witness_identity(witness:bytes)->None:
    if len(witness)<12 or witness[:8] not in WITNESS_MAJORS: raise SystemExit("OMGRFN12 requires OMGRSW1, OMGRSW2, or OMGRSW3")
    major=WITNESS_MAJORS[witness[:8]]
    if struct.unpack_from("<HH",witness,8)!=(major,0): raise SystemExit("OMGRFN12 witness identity mismatch")
def require_ckir10_identity(ckir:bytes)->None:
    if len(ckir)<12 or ckir[:8]!=CKIR_MAGIC or struct.unpack_from("<HH",ckir,8)!=(10,0): raise SystemExit("OMGRFN12 requires CKIR schema 10.0")
def main()->None:
    parser=argparse.ArgumentParser(); parser.add_argument("omgcomp",type=Path); parser.add_argument("witness",type=Path); parser.add_argument("ckir",type=Path); parser.add_argument("elf",type=Path); parser.add_argument("--result",type=int); parser.add_argument("--library",action="store_true"); args=parser.parse_args()
    omgcomp=bounded(args.omgcomp,MAX_OMGCOMP,"OMGCOMP"); witness=bounded(args.witness,MAX_WITNESS,"OMGRSW"); ckir=bounded(args.ckir,MAX_CKIR,"CKIR10"); elf=bounded(args.elf,MAX_ELF,"ELF")
    if not omgcomp or not witness or not ckir: raise SystemExit("OMGCOMP, OMGRSW, and CKIR10 must be nonempty")
    require_witness_identity(witness); require_ckir10_identity(ckir)
    if HEADER.size+len(omgcomp)+len(witness)+len(ckir)+len(elf)>MAX_FRAME: raise SystemExit("OMGRFN12 frame exceeds whole-frame ceiling")
    if args.library:
        if args.result is not None or elf: raise SystemExit("library frame requires no result and empty ELF")
        flags=0; result=exit_code=NO_RESULT
    else:
        if args.result is None or not 0<=args.result<=NO_RESULT or not elf: raise SystemExit("entry frame requires a u32 result and nonempty ELF")
        flags=1; result=args.result; exit_code=result&255
    sys.stdout.buffer.write(HEADER.pack(MAGIC,12,flags,len(omgcomp),len(witness),len(ckir),len(elf),result,exit_code)); sys.stdout.buffer.write(omgcomp+witness+ckir+elf)
if __name__=="__main__": main()
