#!/usr/bin/env python3
# asm_ref.py — an INDEPENDENT reference assembler: Alpha assembly text (stdin) -> raw bytecode tape (stdout),
# written from beta/README.md + the 21-opcode encoding, NOT ported from assembler.alpha.
#
# WHY THIS EXISTS — executable reference and regression coverage for the Alpha
# assembly encoding. asm-diamond.sh assembles real programs with the lattice
# assembler and this implementation and compares their tapes. This tool is
# UNTRUSTED; agreement is diagnostic and does not replace source-to-artifact
# refinement. The runtime lineage never runs it.
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
                if (len(tok) < 2 or tok[0] != 'r' or
                        not tok[1:].isascii() or not tok[1:].isdigit()):
                    raise SyntaxError(f'asm_ref: malformed register {tok!r}')
                value = int(tok[1:])
                if value > 255:
                    raise SyntaxError(f'asm_ref: register out of range {tok!r}')
                out.append(value)
            else:
                v = labels[tok] if tok in labels else int(tok)
                out += (v & MASK).to_bytes(8, 'little')
    return out

def main():
    sys.stdout.buffer.write(assemble(sys.stdin.read()))

main()
