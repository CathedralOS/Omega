#!/usr/bin/env python3
# gamma_ref.py — an INDEPENDENT reference evaluator for Gamma (the meaning substrate), written from
# gamma/LANGUAGE.md + interp.beta's grammar, NOT ported from interp.beta. Reads a Gamma program on stdin,
# prints the signed decimal result, and exits with its low byte (matching interp.beta).
#
# WHY THIS EXISTS — Gamma is where meaning lives: interp.beta is the canonical definition of what programs
# MEAN, and the proof kernel proves theorems ABOUT that meaning, so interp.beta's correctness underpins the proof edifice.
# interp.beta is cross-checked on fixed corpora and (for arithmetic) against the proof kernel's normalizer, but its
# ADT/match/recursion EVALUATION has no independent implementation to diamond against. This is that
# implementation. gamma-diamond-py.sh runs random Gamma programs through BOTH interp.beta and gamma_ref.py
# and asserts they agree. UNTRUSTED and checked, like the other *_ref tools; the runtime never runs it.
#
# Gamma:  E := INT | (+ E E)|(- E E)|(* E E)|(/ E E)|(% E E)|(eq E E)|(lt E E) | (if E E E) | (let x E E)
#              | Con | (Con E...) | (match E (PAT E)...) | (fn E...)      PAT := Con | (Con x...)
# Arithmetic is 64-bit: + - * wrap; / % are signed, truncating toward zero, and trap (div-by-zero, INT_MIN/-1)
# -> exit 132; eq is full 64-bit; lt is signed. Values are ints or constructor nodes.
import sys

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

def tokens(s):
    return s.replace('(', ' ( ').replace(')', ' ) ').split()

def parse(ts, i):
    if ts[i] == '(':
        out = []; i += 1
        while ts[i] != ')':
            node, i = parse(ts, i); out.append(node)
        return out, i + 1
    return ts[i], i + 1

def parse_all(s):
    ts = tokens(s); i = 0; out = []
    while i < len(ts):
        node, i = parse(ts, i); out.append(node)
    return out

ARITH = {'+', '-', '*', '/', '%', 'eq', 'lt'}

class Ev:
    def __init__(self, defs):
        self.defs = defs
        self.steps = 0

    def ev(self, e, env):
        self.steps += 1
        if self.steps > STEP_CAP:
            raise Trap()
        if isinstance(e, str):
            if e.lstrip('-').isdigit():
                return int(e) & MASK
            if e in env:
                return env[e]
            if e[:1].isupper():
                return ('con', e, ())                  # nullary constructor
            raise Trap()                               # unbound name
        head = e[0]
        if head in ARITH:
            a = self.ev(e[1], env); b = self.ev(e[2], env)
            if head == '+':  return (a + b) & MASK
            if head == '-':  return (a - b) & MASK
            if head == '*':  return (a * b) & MASK
            if head == 'eq': return 1 if a == b else 0
            if head == 'lt': return 1 if s64(a) < s64(b) else 0
            sa, sb = s64(a), s64(b)                     # / and %
            if sb == 0 or (sa == INT_MIN and sb == -1):
                raise Trap()
            q = trunc_div(sa, sb)
            return (q if head == '/' else sa - q * sb) & MASK
        if head == 'if':
            return self.ev(e[2], env) if self.ev(e[1], env) != 0 else self.ev(e[3], env)
        if head == 'let':
            env2 = dict(env); env2[e[1]] = self.ev(e[2], env)
            return self.ev(e[3], env2)
        if head == 'match':
            scrut = self.ev(e[1], env)
            if not (isinstance(scrut, tuple) and scrut[0] == 'con'):
                raise Trap()
            _, name, args = scrut
            for pat, body in [(a[0], a[1]) for a in e[2:]]:
                if isinstance(pat, str):
                    if pat[:1].isupper():              # nullary constructor pattern
                        if pat == name and len(args) == 0:
                            return self.ev(body, env)
                    else:                              # variable/catch-all pattern
                        env2 = dict(env); env2[pat] = scrut
                        return self.ev(body, env2)
                elif pat[0] == name and len(pat) - 1 == len(args):
                    env2 = dict(env)
                    for v, val in zip(pat[1:], args):
                        env2[v] = val
                    return self.ev(body, env2)
            raise Trap()                               # no arm matched (non-exhaustive)
        if head[:1].isupper():
            return ('con', head, tuple(self.ev(a, env) for a in e[1:]))
        # function call
        if head not in self.defs:
            raise Trap()
        params, body = self.defs[head]
        env2 = {p: self.ev(a, env) for p, a in zip(params, e[1:])}
        return self.ev(body, env2)

def main():
    forms = parse_all(sys.stdin.read())
    defs = {}
    exprs = []
    for f in forms:
        if isinstance(f, list) and f and f[0] == 'def':
            defs[f[1]] = (f[2], f[3])
        else:
            exprs.append(f)
    ev = Ev(defs)
    try:
        v = ev.ev(exprs[-1], {})
    except Trap:
        sys.exit(132)
    if isinstance(v, tuple):                            # a data result — not used by the int fuzz
        sys.exit(0)
    sys.stdout.write(str(s64(v)))
    sys.exit(v & 0xFF)

main()
