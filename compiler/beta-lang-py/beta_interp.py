#!/usr/bin/env python3
# beta_interp.py SOURCEFILE — an independent REFERENCE INTERPRETER for the Beta language: it runs Beta
# source directly (tree-walking the shared parser's AST) and exits with the
# program's exit code, writing its stdout. It has no dependency on the optional
# bc2 compiler backend.
#
# WHY THIS EXISTS — executable reference meaning for Beta compiler validation.
# Compiler agreement and self-reproduction say nothing about whether bc compiles correctly. Beta has
# no formal spec — bc.beta is its de-facto definition — so this interpreter is a SECOND, independent
# definition of Beta's meaning. `beta-correctness-fuzz.sh` runs random programs both ways — interpret here
# vs. compile-with-bc-and-run-on-the-VM — and asserts they agree, so a bc miscompile surfaces as a
# disagreement. UNTRUSTED and checked, like the rest of the *_ref / *2 tools.
#
# Semantics mirror alpha/SEMANTICS.md exactly (values are 64-bit; comparisons and div/mod are signed,
# truncating toward zero; div-by-zero and INT_MIN/-1 trap; exit code is the low byte of main's result) so
# the interpreter and the compiled-and-run program agree bit for bit.
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from beta_parser import lex, Parser

MASK = (1 << 64) - 1
INT_MIN = -(1 << 63)
STEP_CAP = 20_000_000

class Trap(Exception):
    pass

def s64(x):
    return x - (1 << 64) if x >= (1 << 63) else x

def trunc_div(a, b):
    q = abs(a) // abs(b)
    return -q if (a < 0) != (b < 0) else q

class Interp:
    def __init__(self, procs, stdin_bytes):
        self.procs = {p[1]: p for p in procs}
        self.mem = {}                                  # sparse byte memory (addr -> 0..255)
        self.inp = stdin_bytes
        self.ipos = 0
        self.out = bytearray()
        self.steps = 0

    # ---- memory (little-endian, matching load/store/loadb/storeb) ----
    def loadb(self, a):  return self.mem.get(a & MASK, 0)
    def storeb(self, a, v): self.mem[a & MASK] = v & 0xFF
    def load(self, a):
        return sum(self.mem.get((a + i) & MASK, 0) << (8 * i) for i in range(8))
    def store(self, a, v):
        for i in range(8):
            self.mem[(a + i) & MASK] = (v >> (8 * i)) & 0xFF

    def call(self, name, argvals):
        if name == 'read_byte':
            if self.ipos < len(self.inp):
                b = self.inp[self.ipos]; self.ipos += 1; return b
            return MASK                                # EOF -> all ones (signed -1)
        if name == 'write_byte':
            self.out.append(argvals[0] & 0xFF); return argvals[0]   # r0 unchanged, as in codegen
        if name not in self.procs:
            raise Trap()                               # unknown call
        return self.run_proc(self.procs[name], argvals)

    def ev(self, e, env):
        self.steps += 1
        if self.steps > STEP_CAP:
            raise Trap()
        if e[0] == 'num':
            return e[1] & MASK
        if e[0] == 'var':
            return env[e[1]]
        if e[0] == 'mem':
            a = self.ev(e[2], env)
            return self.loadb(a) if e[1] == 'byte' else self.load(a)
        if e[0] == 'call':
            return self.call(e[1], [self.ev(a, env) for a in e[2]])
        if e[0] == 'bin':
            op = e[1]; a = self.ev(e[2], env); b = self.ev(e[3], env)
            if op == '+':  return (a + b) & MASK
            if op == '-':  return (a - b) & MASK
            if op == '*':  return (a * b) & MASK
            if op in ('/', '%'):
                sa, sb = s64(a), s64(b)
                if sb == 0 or (sa == INT_MIN and sb == -1):
                    raise Trap()
                q = trunc_div(sa, sb)
                return (q if op == '/' else sa - q * sb) & MASK
            if op == '<':  return 1 if s64(a) <  s64(b) else 0
            if op == '>':  return 1 if s64(a) >  s64(b) else 0
            if op == '<=': return 1 if s64(a) <= s64(b) else 0
            if op == '>=': return 1 if s64(a) >= s64(b) else 0
            if op == '==': return 1 if a == b else 0
            if op == '!=': return 1 if a != b else 0
        raise Trap()

    # a proc body is: leading entry statements, then `state` blocks; entry falls into the first state.
    def run_proc(self, proc, argvals):
        _, name, params, body = proc
        env = {}
        for i, p in enumerate(params):
            env[p] = argvals[i] if i < len(argvals) else 0
        # blocks: index 0 = entry stmts (non-state, in order); then one per state, in order.
        blocks = [[]]
        labels = {}
        for s in body:
            if s[0] == 'state':
                labels[s[1]] = len(blocks)
                blocks.append(s[2])
            else:
                blocks[0].append(s)
        pc = 0
        while pc < len(blocks):
            jumped = False
            for st in blocks[pc]:
                self.steps += 1
                if self.steps > STEP_CAP:
                    raise Trap()
                k = st[0]
                if k == 'let' or k == 'assign':
                    env[st[1]] = self.ev(st[2], env)
                elif k == 'return':
                    return self.ev(st[1], env)
                elif k == 'callstmt':
                    self.ev(st[1], env)
                elif k == 'memset':
                    a = self.ev(st[2], env); v = self.ev(st[3], env)
                    (self.storeb if st[1] == 'byte' else self.store)(a, v)
                elif k == 'emit':
                    self.out += decode_str(st[1])
                elif k == 'goto':
                    if st[2] is None or self.ev(st[2], env) != 0:
                        pc = labels[st[1]]; jumped = True; break
                else:
                    raise Trap()
            if not jumped:
                pc += 1                                # fall through to the next state (or off the end)
        return 0                                       # fell off the end (well-formed programs return first)

def decode_str(inner):
    esc = {'n': 10, 't': 9, 'r': 13, '0': 0, '\\': 92, "'": 39, '"': 34}
    out = bytearray(); i = 0
    while i < len(inner):
        if inner[i] == '\\':
            out.append(esc[inner[i + 1]]); i += 2
        else:
            out.append(ord(inner[i])); i += 1
    return out

def main():
    with open(sys.argv[1]) as f:
        procs = Parser(lex(f.read())).parse()
    it = Interp(procs, sys.stdin.buffer.read())
    try:
        rc = it.run_proc(it.procs['main'], [])
    except Trap:
        sys.stdout.buffer.write(it.out); sys.stdout.buffer.flush()
        sys.exit(132)
    sys.stdout.buffer.write(it.out); sys.stdout.buffer.flush()
    sys.exit(rc & 0xFF)

# a reusable entry for exhaustive drivers: interpret `procs` on `stdin_bytes` -> (exit_code, stdout_bytes)
def interpret(procs, stdin_bytes):
    it = Interp(procs, stdin_bytes)
    try:
        rc = it.run_proc(it.procs['main'], [])
    except Trap:
        return 132, bytes(it.out)
    return rc & 0xFF, bytes(it.out)

if __name__ == '__main__':                             # importable (io-verify.py reuses Interp/interpret)
    main()
