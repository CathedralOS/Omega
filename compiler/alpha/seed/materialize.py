#!/usr/bin/env python3
# Materialize a hand-assembled binary from a commented hex listing.
#
# Every opcode/ModRM/SIB/immediate byte is hand-authored in vm.hex; this tool only
# does the address arithmetic a human would otherwise compute by hand and get wrong
# (jump displacements, absolute addresses). That arithmetic is trivial and auditable
# — this is the hex0 -> hex1 step (add labels), not a compiler. Two passes.
#
# Tokens (one or more per line, `;` ends a comment):
#   XX            one literal byte (two hex digits)
#   zN            N zero bytes (decimal N)
#   name:         define a label at the current file offset
#   .rel32 name   4-byte signed (name - here_after) — for jmp/jcc/call rel32
#   .rel8  name   1-byte signed displacement
#   .abs   name   8-byte absolute VA of a .text label (ImageBase + its RVA)
#   .rva   name   4-byte RVA of a .text label
#   .pad   OFF    zero-fill until the output reaches file offset OFF (hex or dec)
import sys

IMAGE_BASE  = 0x140000000
TEXT_FILE   = 0x200     # .text:  file [0x200, 0x1200)  -> RVA 0x1000
RDATA_FILE  = 0x1200    # .rdata: file [0x1200, ...)    -> RVA 0x2000
TEXT_RVA    = 0x1000
RDATA_RVA   = 0x2000

def rva(file_offset):
    if file_offset < RDATA_FILE:
        return file_offset - TEXT_FILE + TEXT_RVA
    return file_offset - RDATA_FILE + RDATA_RVA

SIZED = {'.rel32': 4, '.rel8': 1, '.abs': 8, '.rva': 4}

def assemble(path):
    toks = []
    for raw in open(path):
        toks.extend(raw.split(';', 1)[0].split())

    # pass 1: label offsets
    labels, off, i = {}, 0, 0
    while i < len(toks):
        t = toks[i]
        if t.endswith(':'):
            labels[t[:-1]] = off; i += 1
        elif t[0] in 'zZ':
            off += int(t[1:]); i += 1
        elif t == '.pad':
            off = int(toks[i + 1], 0); i += 2
        elif t in SIZED:
            off += SIZED[t]; i += 2
        else:
            off += 1; i += 1

    # pass 2: emit
    out = bytearray()
    i = 0
    while i < len(toks):
        t = toks[i]
        if t.endswith(':'):
            i += 1
        elif t[0] in 'zZ':
            out += b'\x00' * int(t[1:]); i += 1
        elif t == '.pad':
            out += b'\x00' * (int(toks[i + 1], 0) - len(out)); i += 2
        elif t == '.rel32':
            # RIP-relative in RVA space (works across sections, e.g. call [rip+IAT])
            out += (rva(labels[toks[i + 1]]) - rva(len(out) + 4)).to_bytes(4, 'little', signed=True); i += 2
        elif t == '.rel8':
            disp = labels[toks[i + 1]] - (len(out) + 1)
            if not -128 <= disp <= 127:
                raise SystemExit(f"rel8 to {toks[i+1]} out of range: {disp}")
            out += disp.to_bytes(1, 'little', signed=True); i += 2
        elif t == '.abs':
            out += (IMAGE_BASE + rva(labels[toks[i + 1]])).to_bytes(8, 'little'); i += 2
        elif t == '.rva':
            out += rva(labels[toks[i + 1]]).to_bytes(4, 'little'); i += 2
        else:
            out.append(int(t, 16)); i += 1
    return bytes(out)

if __name__ == '__main__':
    data = assemble(sys.argv[1])
    with open(sys.argv[2], 'wb') as f:
        f.write(data)
    sys.stderr.write(f"{sys.argv[2]}: {len(data)} bytes\n")
