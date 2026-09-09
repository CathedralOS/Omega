#!/usr/bin/env python3
# alpha_ref.py — an INDEPENDENT, auditable reference implementation of the Alpha VM, written straight from
# SEMANTICS.md (the 21-opcode small-step machine). Reads a raw bytecode tape (argv[1]) and the program's
# stdin; writes the program's stdout; exits with the program's exit code (or 132 on a trap, matching the
# seeds' SIGILL -> shell 128+4).
#
# WHY THIS EXISTS — an executable reference for seed conformance. Hand-written
# assembly is hard to audit, so this implementation is short enough to read
# against SEMANTICS.md line by line. It is UNTRUSTED: the cross-check runs real
# and edge-case tapes through this and the host seed. Disagreement is a useful
# diagnostic; agreement does not replace the written semantics or audit.
#
# Encoding (opcode 1 byte; register operand 1 byte; immediate/address 8 bytes LE; address = absolute M
# offset). Loader: M[0..L-1] = tape, pc = 0, R[i] = 0, sp = 0x10000000 (grows down).
import sys

MEMSIZE = 0x70000000              # AlphaBootstrapV4; sp still starts at 0x10000000
TAPE_MAX = 0x00FFFFFC             # 16 MiB stamped hole minus its four-byte length
WIDTHS = (2, 10, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 9, 10, 10, 11, 11, 2, 2, 9, 1)
MASK = (1 << 64) - 1
INT_MIN = -(1 << 63)

def s64(x):                        # interpret a 64-bit word as signed
    return x - (1 << 64) if x >= (1 << 63) else x

def trunc_div(a, b):               # signed division truncated toward zero (C semantics)
    q = abs(a) // abs(b)
    return -q if (a < 0) != (b < 0) else q

def run(tape, stdin_bytes):
    out = bytearray()

    def trap():
        sys.stdout.buffer.write(out)
        sys.stdout.buffer.flush()
        sys.exit(132)              # existing Trap observation, including bounds failures

    def check_range(address, width):
        if not 0 <= address <= MEMSIZE - width:
            trap()

    if len(tape) > TAPE_MAX or len(tape) > MEMSIZE:
        trap()                    # loader failure precedes allocation and copying
    M = bytearray(MEMSIZE)
    M[0:len(tape)] = tape
    R = [0] * 256
    sp = 0x10000000
    pc = 0
    inp = memoryview(stdin_bytes)
    ipos = 0

    def rd8(a):                    # read a little-endian 64-bit word at M[a..a+8]
        check_range(a, 8)
        return int.from_bytes(M[a:a + 8], 'little')
    def wr8(a, v):
        check_range(a, 8)
        M[a:a + 8] = (v & MASK).to_bytes(8, 'little')

    while True:
        check_range(pc, 1)
        op = M[pc]
        if op >= len(WIDTHS):
            trap()
        check_range(pc, WIDTHS[op]) # complete operands before register, memory, or I/O effects
        if op == 0x00:                                   # halt d
            sys.stdout.buffer.write(out); sys.stdout.buffer.flush()
            sys.exit(R[M[pc + 1]] & 0xFF)
        elif op == 0x01:                                 # imm d, k
            R[M[pc + 1]] = rd8(pc + 2); pc += 10
        elif op == 0x02:                                 # mov d, s
            R[M[pc + 1]] = R[M[pc + 2]]; pc += 3
        elif op == 0x03:                                 # add
            d = M[pc + 1]; R[d] = (R[d] + R[M[pc + 2]]) & MASK; pc += 3
        elif op == 0x04:                                 # sub
            d = M[pc + 1]; R[d] = (R[d] - R[M[pc + 2]]) & MASK; pc += 3
        elif op == 0x05:                                 # mul
            d = M[pc + 1]; R[d] = (R[d] * R[M[pc + 2]]) & MASK; pc += 3
        elif op == 0x06 or op == 0x07:                   # div / mod (signed)
            d = M[pc + 1]; a = s64(R[d]); b = s64(R[M[pc + 2]])
            if b == 0 or (a == INT_MIN and b == -1):
                trap()
            q = trunc_div(a, b)
            R[d] = (q if op == 0x06 else a - q * b) & MASK; pc += 3
        elif op == 0x08:                                 # loadb d, s
            check_range(R[M[pc + 2]], 1)
            R[M[pc + 1]] = M[R[M[pc + 2]]]; pc += 3
        elif op == 0x09:                                 # storeb d, s
            check_range(R[M[pc + 1]], 1)
            M[R[M[pc + 1]]] = R[M[pc + 2]] & 0xFF; pc += 3
        elif op == 0x0A:                                 # load d, s  (LE 64-bit)
            R[M[pc + 1]] = rd8(R[M[pc + 2]]); pc += 3
        elif op == 0x0B:                                 # store d, s (LE 64-bit)
            wr8(R[M[pc + 1]], R[M[pc + 2]]); pc += 3
        elif op == 0x0C:                                 # jmp a
            pc = rd8(pc + 1)
        elif op == 0x0D:                                 # jz c, a
            pc = rd8(pc + 2) if R[M[pc + 1]] == 0 else pc + 10
        elif op == 0x0E:                                 # jnz c, a
            pc = rd8(pc + 2) if R[M[pc + 1]] != 0 else pc + 10
        elif op == 0x0F:                                 # jlt a, b, a2  (signed <)
            pc = rd8(pc + 3) if s64(R[M[pc + 1]]) < s64(R[M[pc + 2]]) else pc + 11
        elif op == 0x10:                                 # jeq a, b, a2
            pc = rd8(pc + 3) if R[M[pc + 1]] == R[M[pc + 2]] else pc + 11
        elif op == 0x11:                                 # read d
            if ipos < len(inp):
                R[M[pc + 1]] = inp[ipos]; ipos += 1
            else:
                R[M[pc + 1]] = MASK                       # EOF -> all ones
            pc += 2
        elif op == 0x12:                                 # write s
            out.append(R[M[pc + 1]] & 0xFF); pc += 2
        elif op == 0x13:                                 # call a
            target = rd8(pc + 1)                         # capture before an overlapping push
            wr8(sp - 8, pc + 9); sp -= 8; pc = target
        elif op == 0x14:                                 # ret
            pc = rd8(sp); sp += 8
        else:
            trap()

def main():
    with open(sys.argv[1], 'rb') as f:
        tape = f.read()
    run(tape, sys.stdin.buffer.read())

main()
