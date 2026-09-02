#!/usr/bin/env python3
# beta_ref.py - an INDEPENDENT reference assembler: Beta text (stdin) -> raw bytecode tape (stdout),
# written from ../LANGUAGE.md + the 21-opcode encoding, NOT ported from beta_compiler.beta.
#
# WHY THIS EXISTS - executable reference and regression coverage for the Alpha
# assembly encoding. compiler-diamond.sh assembles real programs with the canonical
# assembler and this implementation and compares their tapes. This tool is
# UNTRUSTED; agreement is diagnostic and does not replace source-to-artifact
# refinement. The runtime lineage never runs it.
#
# Encoding: opcode 1 byte; register operand 1 byte (`rH` or `rHH`); immediate/address operand 8 bytes LE (`0xH...`,
# or a label resolved to its absolute byte offset in the tape). `db "..."` emits the decoded string bytes.
# Comments are `;` to end of line (respecting string quotes); commas are whitespace.
import re
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
ESC = {'0': 0, '\\': 92, '"': 34}
IDENT = re.compile(r'[a-z_][a-z0-9_]*\Z')
HEX_WORD = re.compile(r'0x[0-9a-f]{1,16}\Z')
HEX_REGISTER = re.compile(r'r[0-9a-f]{1,2}\Z')

def tokenize(text):
    """Lex the byte-preserving Latin-1 view of the complete source stream."""
    toks = []; i = 0; n = len(text)
    while i < n:
        c = text[i]
        if c in ' \t\r\n' or c == ',':
            i += 1; continue
        if c == ';':
            i += 1
            while i < n and text[i] not in '\r\n':
                i += 1
            continue
        if c == '"':                                   # a quoted string stays one token
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == '\\' else 1
            if j >= n:
                raise SyntaxError('beta_ref: unterminated db string')
            toks.append((text[i:j + 1], i, j + 1)); i = j + 1
        else:
            j = i
            while (j < n and text[j] not in ' \t\r\n' and
                   text[j] not in ',;'):
                j += 1
            toks.append((text[i:j], i, j)); i = j
    return toks

def decode_str(token):
    if len(token) < 2 or token[0] != '"' or token[-1] != '"':
        raise SyntaxError('beta_ref: db requires one quoted string')
    inner = token[1:-1]
    out = bytearray(); i = 0
    while i < len(inner):
        if inner[i] == '\\':
            if i + 1 >= len(inner) or inner[i + 1] not in ESC:
                raise SyntaxError('beta_ref: unknown db escape')
            out.append(ESC[inner[i + 1]]); i += 2
        else:
            value = ord(inner[i])
            if not 32 <= value < 127:
                raise SyntaxError('beta_ref: non-printable raw db byte')
            out.append(value); i += 1
    return out

def parse(text):
    """-> list of items: ('label', name) | ('ins', mnem, [operand tokens]) | ('db', bytes)"""
    items = []; labels = set(); toks = tokenize(text); k = 0
    while k < len(toks):
        t = toks[k][0]
        if t.endswith(':'):
            name = t[:-1]
            if not IDENT.fullmatch(name):
                raise SyntaxError(f'beta_ref: malformed label {name!r}')
            if name in labels:
                raise SyntaxError(f'beta_ref: duplicate label {name!r}')
            labels.add(name); items.append(('label', name)); k += 1; continue
        if t == 'db':
            if k + 1 >= len(toks):
                raise SyntaxError('beta_ref: db requires one quoted string')
            gap = text[toks[k][2]:toks[k + 1][1]]
            if not gap or any(c not in ' \t\r\n' for c in gap):
                raise SyntaxError('beta_ref: db requires whitespace before its string')
            items.append(('db', decode_str(toks[k + 1][0]))); k += 2; continue
        if t not in OPS:
            raise SyntaxError(f'beta_ref: unknown mnemonic {t!r}')
        kinds = OPS[t][1]
        operands = [token[0] for token in toks[k + 1:k + 1 + len(kinds)]]
        if len(operands) != len(kinds):
            raise SyntaxError(f'beta_ref: missing operand for {t!r}')
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
    for offset, c in enumerate(text):
        value = ord(c)
        if value not in (9, 10, 13) and not 32 <= value <= 126:
            raise SyntaxError(
                f'beta_ref: invalid source byte at offset {offset}'
            )
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
                if not HEX_REGISTER.fullmatch(tok):
                    raise SyntaxError(f'beta_ref: malformed register {tok!r}')
                value = int(tok[1:], 16)
                out.append(value)
            else:
                if HEX_WORD.fullmatch(tok):
                    v = int(tok[2:], 16)
                elif IDENT.fullmatch(tok) and tok in labels:
                    v = labels[tok]
                else:
                    raise SyntaxError(f'beta_ref: malformed or unresolved word {tok!r}')
                out += v.to_bytes(8, 'little')
    return out

def main():
    # Latin-1 is a byte-preserving view; assemble validates every source byte
    # before tokenization, including bytes inside comments.
    sys.stdout.buffer.write(assemble(sys.stdin.buffer.read().decode('latin1')))

main()
