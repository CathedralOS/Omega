#!/usr/bin/env python3
# forge.py — resolve the annotated .hex listing of alpha_x64_windows.exe into
# the binary it audits.  The historical forge is gone; this re-implementation
# exists so the seed's listing remains the buildable source of truth.
#
# Dialect (as committed in bootstrap/0_alpha/alpha_x64_windows.hex):
#   hex pairs            literal bytes, any count per line
#   z<N>                 N zero bytes, N decimal
#   .pad 0x<hex>         zero-fill until absolute file offset <hex>
#   <name>:              label bound to the current file offset
#   .rva <name>          4-byte LE RVA of the label (PE32+ thunk entries pair it
#                        with a literal high dword, e.g. `.rva n_gsh 00 00 00 00`)
#   .rel32 <name>        4-byte LE RVA delta to the label, measured from the end
#                        of the emitted field (call/jump displacement position)
#   .rel8 <name>         1-byte signed delta to the label from end of field
#   ; ...                comment to end of line
#
# Label RVAs resolve through the section headers the listing itself emits:
# the forge parses the emitted section table, maps each label's file offset to
# its section, and applies (rva = fileoff - PointerToRawData + VirtualAddress).
#
# Usage: python3 forge.py <listing.hex> <output.exe>
#        python3 forge.py <listing.hex> --check <committed.exe>   # byte compare

import re
import struct
import sys
import tokenize


def parse_listing(text):
    """Return (ops, labels): ops are ('bytes',bytes)|('zpad',n)|('padto',off)|
    ('rel8',name,off)|('rel32',name,off)|('rva',name,off); labels map name->off."""
    ops = []
    labels = {}
    pos = 0  # current file offset
    toks = []
    for raw in text.splitlines():
        line = raw.split(';', 1)[0]
        toks.extend(line.split())
    i = 0
    while i < len(toks):
        t = toks[i]
        if t.endswith(':'):
            labels[t[:-1]] = pos
        elif t == '.pad':
            target = int(toks[i + 1], 16)
            if target < pos:
                raise ValueError('.pad to %#x while at %#x' % (target, pos))
            ops.append(('padto', target))
            pos = target
            i += 2
            continue
        elif t in ('.rel8', '.rel32', '.rva'):
            ops.append((t[1:], toks[i + 1], pos))
            pos += 1 if t == '.rel8' else 4
            i += 2
            continue
        elif re.fullmatch(r'z\d+', t):
            n = int(t[1:], 10)
            ops.append(('zpad', n))
            pos += n
        elif re.fullmatch(r'[0-9A-Fa-f]{2}', t):
            ops.append(('bytes', bytes([int(t, 16)])))
            pos += 1
        else:
            raise ValueError('unrecognized token %r' % t)
        i += 1
    return ops, labels


def emit(ops):
    out = bytearray()
    pos = 0
    for op in ops:
        k = op[0]
        if k == 'bytes':
            out += op[1]
            pos += len(op[1])
        elif k == 'zpad':
            out += b'\0' * op[1]
            pos += op[1]
        elif k == 'padto':
            out += b'\0' * (op[1] - pos)
            pos = op[1]
        elif k in ('rel8', 'rel32', 'rva'):
            size = 1 if k == 'rel8' else 4
            out += b'\0' * size
            pos += size
    return bytes(out)


def section_map(image):
    """(name -> (rawptr, rva)) and ordered raw ranges from the emitted headers."""
    pe, = struct.unpack_from('<I', image, 0x3C)
    nsec, = struct.unpack_from('<H', image, pe + 4 + 2)
    optsize, = struct.unpack_from('<H', image, pe + 4 + 16)
    shoff = pe + 4 + 20 + optsize
    secs = []
    for i in range(nsec):
        base = shoff + i * 40
        name = image[base:base + 8].rstrip(b'\0').decode()
        va, rawptr = struct.unpack_from('<II', image, base + 12)[0], None
        va, rawsz, rawptr = struct.unpack_from('<III', image, base + 12)
        secs.append((name, va, rawptr, rawsz))
    return secs


def off_to_rva(secs, off):
    for name, va, rawptr, rawsz in secs:
        if rawptr <= off < rawptr + rawsz:
            return va + (off - rawptr)
    raise ValueError('offset %#x not inside any section raw data' % off)


def resolve(image, ops, labels, secs):
    out = bytearray(image)
    for op in ops:
        k = op[0]
        if k not in ('rel8', 'rel32', 'rva'):
            continue
        _, name, off = op
        if name not in labels:
            raise ValueError('undefined label %s' % name)
        tgt = off_to_rva(secs, labels[name])
        if k == 'rva':
            val = tgt
        else:
            site = off_to_rva(secs, off)
            field = 1 if k == 'rel8' else 4
            val = tgt - (site + field)
            if k == 'rel8' and not (-128 <= val <= 127):
                raise ValueError('rel8 to %s out of range: %d' % (name, val))
            if k == 'rel8':
                val &= 0xFF
            if k == 'rel32':
                val &= 0xFFFFFFFF
        fmt = '<B' if k == 'rel8' else '<I'
        struct.pack_into(fmt, out, off, val)
    return bytes(out)


def main(argv):
    text = open(argv[1]).read()
    ops, labels = parse_listing(text)
    image = emit(ops)
    secs = section_map(image)
    image = resolve(image, ops, labels, secs)
    if len(argv) == 3:
        open(argv[2], 'wb').write(image)
        print('wrote %s (%d bytes)' % (argv[2], len(image)))
    elif len(argv) == 4 and argv[2] == '--check':
        committed = open(argv[3], 'rb').read()
        if committed == image:
            print('check OK — listing reproduces the committed binary byte-for-byte')
            return 0
        n = min(len(committed), len(image))
        for i in range(n):
            if committed[i] != image[i]:
                print('first diff at file offset %#x: committed %02x forged %02x'
                      % (i, committed[i], image[i]))
                break
        print('lengths committed %d forged %d' % (len(committed), len(image)))
        return 1
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
