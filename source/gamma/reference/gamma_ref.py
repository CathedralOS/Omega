#!/usr/bin/env python3
# gamma_ref.py - an INDEPENDENT reference evaluator for Gamma (the meaning substrate), written from
# gamma/LANGUAGE.md + interp.beta's grammar, NOT ported from interp.beta. Reads a Gamma program on stdin,
# prints the signed decimal result, and exits with its low byte (matching interp.beta).
#
# WHY THIS EXISTS - interp.beta is the current executable Gamma semantics, but
# its ADT/match/recursion evaluation needs one independent discriminator. This is that
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

class SourceReject(Exception):
    pass

def s64(x):
    return x - (1 << 64) if x >= (1 << 63) else x

def trunc_div(a, b):
    q = abs(a) // abs(b)
    return -q if (a < 0) != (b < 0) else q

def is_ident_start(c):
    return 'A' <= c <= 'Z' or 'a' <= c <= 'z' or c == '_'

def is_ident_continue(c):
    return is_ident_start(c) or '0' <= c <= '9'

def tokens(source):
    if isinstance(source, bytes):
        source = source.decode('latin1')
    for offset, c in enumerate(source):
        value = ord(c)
        if value not in (9, 10, 13) and not 32 <= value <= 126:
            raise SourceReject(f'invalid source byte at offset {offset}')

    out = []
    i, n = 0, len(source)
    while i < n:
        c = source[i]
        if c in ' \t\r\n':
            i += 1
            continue
        if c == ';':
            i += 1
            while i < n and source[i] not in '\r\n':
                i += 1
            continue
        if c in '()':
            out.append(c)
            i += 1
            continue
        j = i
        while j < n and source[j] not in ' \t\r\n();':
            j += 1
        token = source[i:j]
        if not (
            token in {'+', '-', '*', '/', '%'} or
            token and all('0' <= digit <= '9' for digit in token) or
            token and is_ident_start(token[0]) and
            all(is_ident_continue(char) for char in token[1:])
        ):
            raise SourceReject(f'invalid token at offset {i}')
        out.append(token)
        i = j
    return out

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
            if e and all('0' <= digit <= '9' for digit in e):
                return int(e) & MASK
            if e in env:
                return env[e]
            if e[:1] and 'A' <= e[0] <= 'Z':
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
                    if pat[:1] and 'A' <= pat[0] <= 'Z':  # nullary constructor pattern
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
        if head[:1] and 'A' <= head[0] <= 'Z':
            return ('con', head, tuple(self.ev(a, env) for a in e[1:]))
        # function call
        if head not in self.defs:
            raise Trap()
        params, body = self.defs[head]
        env2 = {p: self.ev(a, env) for p, a in zip(params, e[1:])}
        return self.ev(body, env2)

def main():
    try:
        forms = parse_all(sys.stdin.buffer.read())
    except SourceReject:
        sys.exit(255)
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
