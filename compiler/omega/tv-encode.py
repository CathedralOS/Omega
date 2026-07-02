#!/usr/bin/env python3
# tv-encode.py — TRANSLATION VALIDATION, slice 0: encode a straight-line program's MEANING as a
# delta arithmetic term, so the trust anchor itself recomputes it.
#
#   usage: tv-encode.py <claimed-exit>   (the gamma term from omega2gamma.beta on stdin)
#
# Input: the meaning route's gamma term for a STRAIGHT-LINE +/* program:
#   (def m0_me (l0 ...) (let t0 <e> (let t1 <e> ... <result>))) (m0_me 0 ...)
# Output: a delta certificate
#   (= <term-with-lets-inlined, + as p, * as m, ints unary> <unary claimed-exit>)  (refl <unary claimed-exit>)
#
# check.beta ACCEPTS iff the claimed exit REALLY IS the program's meaning — delta's conversion rule
# re-evaluates the arithmetic inside the kernel. Feed it the exit the NATIVE binary actually produced:
# acceptance certifies this compilation agreed with the source's meaning; a miscompiled binary's exit
# is unreachable by conversion and is REJECTED. This tool is UNTRUSTED (like prover.py): a bad encoding
# can only produce certs that fail, or mis-state the meaning — and meaning-fidelity is cross-checked by
# the kernel diamond (the same gamma term must reproduce native behavior on the test corpus).
#
# Slice 0 scope: + and * only (delta's p/m), lets, int literals, locals (zero-init). Anything else ->
# exit 2 ("outside subset") so the gate can skip rather than lie.
import sys

def tokens(s):
    return s.replace('(', ' ( ').replace(')', ' ) ').split()

def parse(ts, i=0):
    if ts[i] == '(':
        out = []
        i += 1
        while ts[i] != ')':
            node, i = parse(ts, i)
            out.append(node)
        return out, i + 1
    return ts[i], i + 1

def unary(n):
    if n < 0:
        sys.exit(2)
    t = 'z'
    for _ in range(n):
        t = f'(s {t})'
    return t

def enc(e, env):
    if isinstance(e, str):
        if e.lstrip('-').isdigit():
            return unary(int(e))
        if e in env:
            return env[e]
        sys.exit(2)                    # an unbound name -> outside the subset
    if len(e) == 3 and e[0] in ('+', '*'):
        op = 'p' if e[0] == '+' else 'm'
        return f'({op} {enc(e[1], env)} {enc(e[2], env)})'
    if len(e) == 4 and e[0] == 'let':
        env2 = dict(env)
        env2[e[1]] = enc(e[2], env)    # inline the binding (straight-line -> pure substitution)
        return enc(e[3], env2)
    sys.exit(2)                        # -, /, %, if, match, calls ... -> later slices

def main():
    claimed = int(sys.argv[1])
    src = sys.stdin.read()
    prog, _ = parse(tokens(src))
    # top level is a single (def m0_me (params...) body) followed by the call; find them
    forms = []
    ts = tokens(src)
    i = 0
    while i < len(ts):
        node, i = parse(ts, i)
        forms.append(node)
    defs = [f for f in forms if isinstance(f, list) and f and f[0] == 'def']
    call = forms[-1]
    if len(defs) != 1 or not isinstance(call, list):
        sys.exit(2)                    # multiple machines / states -> later slices
    _, name, params, body = defs[0]
    args = call[1:]
    if call[0] != name or len(args) != len(params):
        sys.exit(2)
    env = {p: enc(a, {}) for p, a in zip(params, args)}
    term = enc(body, env)
    e = unary(claimed)
    print(f'(= {term} {e}) (refl {e})')

main()
