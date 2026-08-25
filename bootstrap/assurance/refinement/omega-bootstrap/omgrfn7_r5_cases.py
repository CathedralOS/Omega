#!/usr/bin/env python3
"""Focused immutable-frame mutations for OMGRFN7 responsibility 5."""
import argparse, struct
from pathlib import Path

def main():
    p=argparse.ArgumentParser(); p.add_argument("mode"); p.add_argument("source",type=Path); p.add_argument("output",type=Path)
    a=p.parse_args(); raw=bytearray(a.source.read_bytes())
    omg,wit,ckir=struct.unpack_from("<III",raw,16); c=40+omg+wit
    if a.mode=="claim71": struct.pack_into("<II",raw,32,71,71)
    elif a.mode=="result-opaque": struct.pack_into("<II",raw,32,71,71)
    elif a.mode=="source-opaque": raw[40]^=0x55
    elif a.mode=="witness-opaque": raw[40+omg+6]^=0x55
    elif a.mode=="version6": raw[6]=ord("6"); struct.pack_into("<I",raw,8,6)
    elif a.mode=="bad-tag":
        # First opcode-14 immediate tag belongs to case 1; redirect to case 2
        counts=struct.unpack_from("<19I",raw,c+16); cur=c+100
        sizes=(24,20,16,20,20,16,36,20,32,20,24,4)
        for n,s in zip(counts[2:14],sizes): cur+=n*s
        for i in range(counts[14]):
            at=cur+i*40
            if raw[at+12]==14: struct.pack_into("<I",raw,at+32,2); break
    elif a.mode=="cases4097": struct.pack_into("<I",raw,c+40,4097)
    elif a.mode=="ckir-constant":
        counts=struct.unpack_from("<19I",raw,c+16); cur=c+100
        sizes=(24,20,16,20,20,16,36,20,32,20,24,4)
        for n,s in zip(counts[2:14],sizes): cur+=n*s
        for i in range(counts[14]):
            at=cur+i*40
            if raw[at+12]==1: struct.pack_into("<I",raw,at+32,struct.unpack_from("<I",raw,at+32)[0]^1); break
    elif a.mode=="elf-byte": raw[-1]^=1
    elif a.mode=="truncated": raw=raw[:-1]
    elif a.mode=="trailing": raw+=b"\0"
    elif a.mode=="extract-ckir": a.output.write_bytes(raw[c:c+ckir]); return
    else: raise SystemExit("unknown mutation")
    a.output.write_bytes(raw)
if __name__=="__main__": main()
