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

ONE = ('s', ('z',))

# ---- linear-loop analysis: read a per-iteration increment off ONE symbolic body execution ----------
def _mentions(t, ph):                  # does the placeholder `ph` occur in term `t`?
    if t == ph:
        return True
    return isinstance(t, tuple) and t[0] in ('s', 'p', 'm') and any(_mentions(x, ph) for x in t[1:])

def _lin_delta(expr, ph):
    """expr = ph + D or D + ph (D free of ph) -> D (the per-iteration increment); expr == ph -> 0; else None."""
    if expr == ph:
        return 0
    if isinstance(expr, tuple) and expr[0] == 'p':
        a, b = expr[1], expr[2]
        if a == ph and not _mentions(b, ph):
            return b
        if b == ph and not _mentions(a, ph):
            return a
    return None

def _mentions_loopvar(t, names):       # does term `t` mention any ('loopvar', v) with v in names?
    if isinstance(t, tuple):
        if t[0] == 'loopvar':
            return t[1] in names
        if t[0] in ('s', 'p', 'm'):
            return any(_mentions_loopvar(x, names) for x in t[1:])
    return False

def _expr_uses(e, names):              # does AST expression `e` reference any of the given variable names?
    if e[0] == 'var':
        return e[1] in names
    if e[0] == 'bin':
        return _expr_uses(e[2], names) or _expr_uses(e[3], names)
    if e[0] == 'call':
        return any(_expr_uses(a, names) for a in e[2])
    if e[0] == 'mem':
        return _expr_uses(e[2], names)
    return False

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

    def _summarize_loop(self, header_pc, body_label, cond, env, blocks, labels):
        """Recognize a linear counter loop  `state H { to B when (i<n) <exit> } state B { <lin updates> to H }`
        and replace it by the closed form of its accumulators, so a SYMBOLIC trip count is handled WITHOUT
        unrolling. The per-iteration increments are read off ONE exact symbolic body execution (each loop var
        set to a fresh placeholder); an accumulator with a loop-invariant delta d over trip count t becomes
        init + t*d. Returns True (env mutated to the post-loop state) iff the strict pattern matched; else the
        caller re-raises Unsupported. Non-unit strides, self-referential deltas (Σi), and non-zero counter
        starts are out of scope here — deliberately narrow so the closed form is exactly built-in +/*."""
        if body_label not in labels:
            return False
        body = blocks[labels[body_label]]
        if not body or body[-1][0] != 'goto' or body[-1][2] is not None or labels.get(body[-1][1]) != header_pc:
            return False                                # body must end with an unconditional jump back here
        updates = body[:-1]
        if any(s[0] not in ('let', 'assign') for s in updates):
            return False                                # body is straight-line assignments only
        loop_vars = [s[1] for s in updates]
        entry = {v: env.get(v, 0) for v in loop_vars}
        saved = dict(env)                               # read deltas off one body run with fresh placeholders
        for v in loop_vars:
            env[v] = ('loopvar', v)
        try:
            for s in updates:
                env[s[1]] = self.ev(s[2], env)
            deltas = {v: _lin_delta(env[v], ('loopvar', v)) for v in loop_vars}
        except Unsupported:
            deltas = None
        env.clear(); env.update(saved)                  # restore the entry state
        if deltas is None or any(d is None for d in deltas.values()):
            return False
        if cond[0] != 'bin' or cond[1] not in ('<', '<=') or cond[2][0] != 'var':
            return False
        counter = cond[2][1]
        if counter not in loop_vars or deltas[counter] != ONE or entry[counter] != 0:
            return False                                # counter: a unit-stride loop var starting at 0
        if _expr_uses(cond[3], loop_vars):
            return False                                # the bound must be loop-invariant
        try:
            bound = self.ev(cond[3], env)
        except Unsupported:
            return False
        trip = bound if cond[1] == '<' else _add(bound, 1)      # iterations: 0..bound-1 (<) or 0..bound (<=)
        for v in loop_vars:
            if v == counter:
                env[v] = trip
            elif _mentions_loopvar(deltas[v], loop_vars):
                return False                            # Σi-style delta (depends on another loop var): later slice
            else:
                env[v] = _add(entry[v], _mul(trip, deltas[v]))
        return True

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
                    take = st[2] is None
                    if st[2] is not None:
                        try:
                            take = _concrete(self.ev(st[2], env), 'branch on symbolic value') != 0
                        except Unsupported:            # symbolic guard: try to summarize a data-dependent loop
                            if self._summarize_loop(pc, st[1], st[2], env, blocks, labels):
                                take = False           # loop replaced by its closed form; fall to the exit
                            else:
                                raise
                    if take:
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
