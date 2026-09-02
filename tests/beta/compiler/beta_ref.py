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
# Encoding: opcode 1 byte; register operand 1 byte (`rH` or `rHH`);
# immediate/address operand 8 bytes LE (`0xH...`). `0xH...:` asserts the
# current output offset. `dw 0xH...` emits one eight-byte little-endian word.
# Comments are `;` to end of line; commas are whitespace.
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
HEX_WORD = re.compile(r'0x[0-9a-f]{1,16}\Z')
ADDRESS_ASSERTION = re.compile(r'0x[0-9a-f]{1,16}:\Z')
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
        j = i
        while (j < n and text[j] not in ' \t\r\n' and
               text[j] not in ',;'):
            j += 1
        toks.append((text[i:j], i, j)); i = j
    return toks

def parse(text):
    """-> list of address assertions, instructions, or byte data."""
    items = []; toks = tokenize(text); k = 0
    while k < len(toks):
        t = toks[k][0]
        if t.endswith(':'):
            if not ADDRESS_ASSERTION.fullmatch(t):
                raise SyntaxError(f'beta_ref: malformed address assertion {t!r}')
            items.append(('assert', int(t[2:-1], 16))); k += 1; continue
        if t == 'dw':
            if k + 1 >= len(toks):
                raise SyntaxError('beta_ref: dw requires one word')
            word = toks[k + 1][0]
            if not HEX_WORD.fullmatch(word):
                raise SyntaxError(f'beta_ref: malformed dw operand {word!r}')
            items.append(('dw', int(word[2:], 16))); k += 2; continue
        if t not in OPS:
            raise SyntaxError(f'beta_ref: unknown mnemonic {t!r}')
        kinds = OPS[t][1]
        operands = [token[0] for token in toks[k + 1:k + 1 + len(kinds)]]
        if len(operands) != len(kinds):
            raise SyntaxError(f'beta_ref: missing operand for {t!r}')
        items.append(('ins', t, operands)); k += 1 + len(kinds)
    return items

def size(item):
    if item[0] == 'assert':
        return 0
    if item[0] == 'dw':
        return 8
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
    out = bytearray()
    for it in items:
        if it[0] == 'assert':
            if it[1] != len(out):
                raise SyntaxError(
                    f'beta_ref: address assertion {it[1]:#x} at {len(out):#x}'
                )
            continue
        if it[0] == 'dw':
            out += it[1].to_bytes(8, 'little'); continue
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
                else:
                    raise SyntaxError(f'beta_ref: malformed word {tok!r}')
                out += v.to_bytes(8, 'little')
    return out

def main():
    # Latin-1 is a byte-preserving view; assemble validates every source byte
    # before tokenization, including bytes inside comments.
    sys.stdout.buffer.write(assemble(sys.stdin.buffer.read().decode('latin1')))

main()
