#!/usr/bin/env python3
# vm-fuzz-gen.py SEED — emit ONE random, valid, terminating Alpha tape (raw bytecode) to stdout, chosen
# deterministically from SEED. Used by vm-fuzz.sh to differential-test the hand-authored seed VMs against
# the independent reference alpha_ref.py: three implementations of the same 21-op semantics must agree on
# every tape, so a divergence exposes a VM or reference-model bug.
#
# Scope = ARITHMETIC: imm / mov / add / sub / mul / div / mod / halt. Straight-line (no jumps, no memory,
# no I/O) so every tape terminates and stays in bounds by construction — the point is to hammer the
# 64-bit wraparound + SIGNED div/mod (truncate-toward-zero) + trap (div-by-zero, INT_MIN/-1) edges, which
# is exactly where two native implementations most easily disagree. A trapping op is fine: all three impls
# must trap identically (shell 132). Control-flow / memory / I/O are covered by the real-program corpus in
# diamond-py.sh.
import sys, random

MASK = (1 << 64) - 1
# a value pool skewed toward the edges signed 64-bit arithmetic gets wrong
POOL = [0, 1, 2, 3, 7, 10, 0xFF, 1000,
        MASK,                       # -1
        MASK - 1,                   # -2
        (1 << 63),                  # INT_MIN
        (1 << 63) - 1,              # INT_MAX
        (1 << 63) + 1,              # INT_MIN + 1
        MASK - 6,                   # -7
        (1 << 32), (1 << 32) - 1]

def imm(d, k):   return bytes([0x01, d]) + (k & MASK).to_bytes(8, 'little')
def binop(o, d, s): return bytes([o, d, s])
def halt(d):     return bytes([0x00, d])

def main():
    rng = random.Random(int(sys.argv[1]))
    nregs = rng.randint(3, 6)
    regs = list(range(nregs))
    tape = bytearray()
    for d in regs:                                     # seed each register
        k = rng.choice(POOL) if rng.random() < 0.7 else rng.getrandbits(64)
        tape += imm(d, k)
    for _ in range(rng.randint(6, 18)):                # a straight-line run of arithmetic
        o = rng.choice([0x02, 0x03, 0x04, 0x05, 0x06, 0x07])   # mov add sub mul div mod
        tape += binop(o, rng.choice(regs), rng.choice(regs))
    tape += halt(rng.choice(regs))
    sys.stdout.buffer.write(tape)

main()
