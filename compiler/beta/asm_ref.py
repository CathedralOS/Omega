#!/usr/bin/env python3
# asm_ref.py — an INDEPENDENT reference assembler: Alpha assembly text (stdin) -> raw bytecode tape (stdout),
# written from beta/README.md + the 21-opcode encoding, NOT ported from assembler.alpha.
#
# WHY THIS EXISTS — the assembler is a single-implementation gap. bc now has an independent second front end
# (../beta-lang-py/bc2.py) and the VM has an independent reference (../alpha/alpha_ref.py), but the assembler
# `assembler.alpha` has only itself: it self-hosts, and both seeds run the SAME assembler, so a backdoor in
# it would not be caught by the seed diamond. This is a third, independent realization; asm-diamond.sh
# assembles real programs with BOTH the real assembler and this one and asserts byte-identical tapes. It is
# UNTRUSTED and checked (like bc2.py / alpha_ref.py); the runtime lineage never runs it. Together bc2.py +
# asm_ref.py + alpha_ref.py form a complete independent Python realization of the whole alpha->beta->bc floor.
#
# Encoding: opcode 1 byte; register operand 1 byte (`rN`); immediate/address operand 8 bytes LE (a decimal,
# or a label resolved to its absolute byte offset in the tape). `db "..."` emits the decoded string bytes.
# Comments are `;` to end of line (respecting string quotes); commas are whitespace.
import sys

# mnemonic -> (opcode, operand kinds)  where 'r' = register byte, 'x' = 8-byte immediate/address
OPS = {
    'halt': (0x00, 'r'),  'imm': (0x01, 'rx'),
    'mov': (0x02, 'rr'),  'add': (0x03, 'rr'), 'sub': (0x04, 'rr'), 'mul': (0x05, 'rr'),
    'div': (0x06, 'rr'),  'mod': (0x07, 'rr'), 'loadb': (0x08, 'rr'), 'storeb': (0x09, 'rr'),
    'load': (0x0A, 'rr'), 'store': (0x0B, 'rr'),
    'jmp': (0x0C, 'x'),   'jz': (0x0D, 'rx'), 'jnz': (0x0E, 'rx'),
    'jlt': (0x0F, 'rrx'), 'jeq': (0x10, 'rrx'),
    'read': (0x11, 'r'),  'write': (0x12, 'r'), 'call': (0x13, 'x'), 'ret': (0x14, ''),
}
MASK = (1 << 64) - 1
ESC = {'n': 10, 't': 9, 'r': 13, '0': 0, '\\': 92, "'": 39, '"': 34}

def strip_comment(line):
    out = []; q = False; i = 0
    while i < len(line):
        c = line[i]
        if c == '"':
            q = not q
        elif c == ';' and not q:
            break
        out.append(c); i += 1
    return ''.join(out)

def tokenize(line):
    toks = []; i = 0; n = len(line)
    while i < n:
        c = line[i]
        if c in ' \t\r,':
            i += 1; continue
        if c == '"':                                   # a quoted string stays one token
            j = i + 1
            while j < n and line[j] != '"':
                j += 2 if line[j] == '\\' else 1
            toks.append(line[i:j + 1]); i = j + 1
        else:
            j = i
            while j < n and line[j] not in ' \t\r,':
                j += 1
            toks.append(line[i:j]); i = j
    return toks

def decode_str(inner):                                 # inner = text between the quotes
    out = bytearray(); i = 0
    while i < len(inner):
        if inner[i] == '\\':
            out.append(ESC[inner[i + 1]]); i += 2
        else:
            out.append(ord(inner[i])); i += 1
    return out

def parse(text):
    """-> list of items: ('label', name) | ('ins', mnem, [operand tokens]) | ('db', bytes)"""
    items = []
    for raw in text.splitlines():
        toks = tokenize(strip_comment(raw))
        k = 0
        while k < len(toks):
            t = toks[k]
            if t.endswith(':'):
                items.append(('label', t[:-1])); k += 1; continue
            if t == 'db':
                items.append(('db', decode_str(toks[k + 1][1:-1]))); k += 2; continue
            if t not in OPS:
                raise SyntaxError(f'asm_ref: unknown mnemonic {t!r}')
            kinds = OPS[t][1]
            operands = toks[k + 1:k + 1 + len(kinds)]
            items.append(('ins', t, operands)); k += 1 + len(kinds)
    return items

def size(item):
    if item[0] == 'label':
        return 0
    if item[0] == 'db':
        return len(item[1])
    kinds = OPS[item[1]][1]
    return 1 + sum(1 if kd == 'r' else 8 for kd in kinds)

def assemble(text):
    items = parse(text)
    # pass 1: label -> byte offset
    labels = {}; off = 0
    for it in items:
        if it[0] == 'label':
            labels[it[1]] = off
        off += size(it)
    # pass 2: emit
    out = bytearray()
    for it in items:
        if it[0] == 'label':
            continue
        if it[0] == 'db':
            out += it[1]; continue
        op, kinds = OPS[it[1]]
        out.append(op)
        for kd, tok in zip(kinds, it[2]):
            if kd == 'r':
                out.append(int(tok[1:]) & 0xFF)        # rN
            else:
                v = labels[tok] if tok in labels else int(tok)
                out += (v & MASK).to_bytes(8, 'little')
    return out

def main():
    sys.stdout.buffer.write(assemble(sys.stdin.read()))

main()
