#!/usr/bin/env python3
# beta_symbolic.py SOURCEFILE — SYMBOLIC evaluation of a Beta program: the source-side dual of
# alpha/alpha_symbolic.py. It tree-walks bc2.py's AST over a mix of CONCRETE integers and SYMBOLIC Peano
# terms — each read_byte() is a fresh input variable — and reports what function of its inputs the SOURCE
# denotes, as a closed-form expression. Where beta_interp.py RUNS a Beta program on concrete input, this asks
# "what does the source MEAN, for all inputs?" without picking any.
#
# WHY — it turns instruction-level refinement into FULLY AUTOMATIC translation validation. alpha_symbolic
# derives what the COMPILED machine code computes; this derives what the SOURCE means; the refinement gate
# proves the two agree for ALL inputs (prover.py -> check.beta). Both derivations are independent and
# UNTRUSTED — this one is differentially pinned against beta_interp.py's concrete runs, exactly as the *_ref
# tools are — so a bc miscompile OR a bug in either symbolic evaluator surfaces as a rejected proof or a
# differential disagreement. No human writes the "claimed meaning" any more.
#
# Scope mirrors alpha_symbolic: the concrete-control, non-negative-arithmetic fragment. Control flow that
# branches on a SYMBOLIC value (input-dependent loop bound), symbolic subtraction, div/mod, or symbolic memory
# raises Unsupported. Concrete-bounded loops/recursion UNROLL. read_byte order fixes the input numbering.
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from bc2 import lex, Parser

NAT_CAP = 1 << 20
MASK = (1 << 64) - 1

class Unsupported(Exception):
    pass

# ---- Peano terms, rendered to the same syntax alpha_symbolic / the prover use --------------------------
def nat(k):
    if k < 0 or k > NAT_CAP:
        raise Unsupported('constant %d too large for a Peano term' % k)
    t = ('z',)
    for _ in range(k):
        t = ('s', t)
    return t

def render(t):
    h = t[0]
    if h == 'z':  return 'z'
    if h == 's':  return '(s %s)' % render(t[1])
    if h == 'v':  return '(v %d)' % t[1]
    return '(%s %s %s)' % (h, render(t[1]), render(t[2]))

def evaluate(t, env):                  # concrete value under {var_index: int}
    h = t[0]
    if h == 'z':  return 0
    if h == 's':  return 1 + evaluate(t[1], env)
    if h == 'v':  return env[t[1]]
    if h == 'p':  return evaluate(t[1], env) + evaluate(t[2], env)
    return evaluate(t[1], env) * evaluate(t[2], env)

def _term(v):
    return nat(v) if isinstance(v, int) else v

def _add(a, b):
    return (a + b) & MASK if isinstance(a, int) and isinstance(b, int) else ('p', _term(a), _term(b))

def _mul(a, b):
    return (a * b) & MASK if isinstance(a, int) and isinstance(b, int) else ('m', _term(a), _term(b))

def _sub(a, b):
    if isinstance(a, int) and isinstance(b, int):
        return (a - b) & MASK
    raise Unsupported('symbolic subtraction (needs ZZ integers)')

def _concrete(v, why):
    if not isinstance(v, int):
        raise Unsupported(why)
    return v

class SymInterp:
    def __init__(self, procs):
        self.procs = {p[1]: p for p in procs}
        self.n_inputs = 0
        self.steps = 0

    def ev(self, e, env):
        self.steps += 1
        if self.steps > 2_000_000:
            raise Unsupported('step budget (a data-independent loop?)')
        k = e[0]
        if k == 'num':
            return e[1] & MASK
        if k == 'var':
            return env[e[1]]
        if k == 'call':
            name = e[1]
            if name == 'read_byte':
                v = ('v', self.n_inputs); self.n_inputs += 1; return v      # a fresh input variable
            if name == 'write_byte':
                return self.ev(e[2][0], env)                                # value flows through unchanged
            if name not in self.procs:
                raise Unsupported('unknown call %s' % name)
            return self.run(self.procs[name], [self.ev(a, env) for a in e[2]])
        if k == 'bin':
            op = e[1]; a = self.ev(e[2], env); b = self.ev(e[3], env)
            if op == '+':  return _add(a, b)
            if op == '*':  return _mul(a, b)
            if op == '-':  return _sub(a, b)
            # comparisons: only decidable on concretes (that is what keeps control flow concrete)
            x = _concrete(a, 'compare on symbolic value'); y = _concrete(b, 'compare on symbolic value')
            from_signed = lambda z: z - (1 << 64) if z >= (1 << 63) else z
            sx, sy = from_signed(x), from_signed(y)
            return 1 if {'<': sx < sy, '>': sx > sy, '<=': sx <= sy, '>=': sx >= sy,
                         '==': x == y, '!=': x != y}[op] else 0
        raise Unsupported('expression form %s' % k)     # 'mem' (byte[]/word[]) not modelled yet

    def run(self, proc, argvals):
        _, name, params, body = proc
        env = {p: (argvals[i] if i < len(argvals) else 0) for i, p in enumerate(params)}
        blocks = [[]]; labels = {}
        for s in body:
            if s[0] == 'state':
                labels[s[1]] = len(blocks); blocks.append(s[2])
            else:
                blocks[0].append(s)
        pc = 0
        while pc < len(blocks):
            jumped = False
            for st in blocks[pc]:
                self.steps += 1
                if self.steps > 2_000_000:
                    raise Unsupported('step budget (a data-independent loop?)')
                k = st[0]
                if k == 'let' or k == 'assign':
                    env[st[1]] = self.ev(st[2], env)
                elif k == 'return':
                    return self.ev(st[1], env)
                elif k == 'callstmt':
                    self.ev(st[1], env)
                elif k == 'goto':
                    if st[2] is None or _concrete(self.ev(st[2], env), 'branch on symbolic value') != 0:
                        pc = labels[st[1]]; jumped = True; break
                else:
                    raise Unsupported('statement form %s' % k)   # memset/emit not modelled yet
            if not jumped:
                pc += 1
        return 0

def meaning(src_text):
    """-> (output_term, n_inputs): the closed-form meaning of `main`, over inputs (v 0)..(v n-1)."""
    procs = Parser(lex(src_text)).parse()
    si = SymInterp(procs)
    out = si.run(si.procs['main'], [])
    return _term(out), si.n_inputs

def main():
    with open(sys.argv[1]) as f:
        out, n = meaning(f.read())
    sys.stdout.write('%d %s\n' % (n, render(out)))

if __name__ == '__main__':
    main()
