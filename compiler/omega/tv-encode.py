#!/usr/bin/env python3
# tv-encode.py — TRANSLATION VALIDATION encoder: turn a straight-line program's MEANING into a delta
# certificate the trust anchor RE-EVALUATES.
#
#   usage: tv-encode.py <claimed-exit>   (the gamma term from omega2gamma.beta on stdin)
#
# Emits:  <arith prelude>  (= <meaning as user-fn arithmetic over user-Nat> <unary claimed-exit>)  (refl <…>)
# check.beta ACCEPTS iff <claimed-exit> really is the program's meaning — delta's conversion rule reduces
# the arithmetic INSIDE the kernel (the encoder only transcribes; it does not decide the answer). Feed the
# exit the NATIVE binary actually produced: acceptance certifies this compilation agreed with the source's
# meaning; a miscompiled binary's exit is unreachable by conversion and is REJECTED.
#
# All arithmetic is delta USER FUNCTIONS over a user-Nat (Z=cid2, S=cid3): uadd(21), usub(22, TRUNCATED
# monus — matches native i32 only while results stay >= 0), umul(23), upred(20). So + - * are all
# kernel-evaluated. `-` is honest monus: a program whose subtraction would go negative has a different
# meaning than native i32 wrap, so the encoder BAILS (exit 2) on it, as it does on any value it cannot
# encode small (unary numerals hit delta's arena wall — guard at MAXV) or any construct outside the
# straight-line + - * subset (/, %, comparisons, loops, calls -> later slices).
#
# UNTRUSTED, like prover.py: a bad encoding can only make certs that FAIL or mis-state the meaning;
# meaning-fidelity is independently pinned by the kernel diamond over the same translator output.
import sys

MAXV = 80   # arena guard: unary numerals beyond this blow delta's node budget

PRELUDE = (
    "(data 2 0 0 0) (data 3 1 1 0) "
    "(fun 20 2 (k 2)) (fun 20 3 (v 0)) "                # upred:  pred Z = Z ; pred (S k) = k
    "(fun 21 2 (y 0)) (fun 21 3 (k 3 (rec 0))) "        # uadd:   add(Z,y)=y ; add(S x,y)=S(add(x,y))
    "(fun 22 2 (y 0)) (fun 22 3 (f 20 (rec 0))) "       # usub:   sub(Z,a)=a ; sub(S k,a)=pred(sub(k,a))  [a-b via sub(b,a)]
    "(fun 23 2 (k 2)) (fun 23 3 (f 21 (y 0) (rec 0)))"  # umul:   mul(Z,y)=Z ; mul(S x,y)=add(y,mul(x,y))
)

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

def parse_all(s):
    ts = tokens(s); i = 0; out = []
    while i < len(ts):
        node, i = parse(ts, i); out.append(node)
    return out

def unary(n):
    if n < 0 or n > MAXV:
        sys.exit(2)
    t = '(k 2)'
    for _ in range(n):
        t = f'(k 3 {t})'
    return t

# evaluate to (delta-term, concrete-value); the value drives the range/monus guards only.
def ev(e, env):
    if isinstance(e, str):
        if e.lstrip('-').isdigit():
            n = int(e)
            return unary(n), n
        if e in env:
            return env[e]
        sys.exit(2)                                        # unbound name -> outside subset
    if len(e) == 3 and e[0] in ('+', '*', '-'):
        (ta, va), (tb, vb) = ev(e[1], env), ev(e[2], env)
        if e[0] == '+':
            v = va + vb; t = f'(f 21 {ta} {tb})'
        elif e[0] == '*':
            v = va * vb; t = f'(f 23 {ta} {tb})'
        else:                                              # a - b  ==  usub(b, a)
            if va < vb:
                sys.exit(2)                                # would go negative: monus != native i32
            v = va - vb; t = f'(f 22 {tb} {ta})'
        if v > MAXV:
            sys.exit(2)
        return t, v
    if len(e) == 4 and e[0] == 'let':
        env2 = dict(env)
        env2[e[1]] = ev(e[2], env)                         # straight-line -> inline the binding
        return ev(e[3], env2)
    sys.exit(2)                                            # /, %, if, match, call, ... -> later slices

def main():
    claimed = int(sys.argv[1])
    forms = parse_all(sys.stdin.read())
    defs = [f for f in forms if isinstance(f, list) and f and f[0] == 'def']
    call = forms[-1]
    if len(defs) != 1 or not isinstance(call, list):
        sys.exit(2)                                        # multiple machines / states -> later slices
    _, name, params, body = defs[0]
    args = call[1:]
    if call[0] != name or len(args) != len(params):
        sys.exit(2)
    env = {p: ev(a, {}) for p, a in zip(params, args)}
    term, _ = ev(body, env)
    e = unary(claimed)
    print(f'{PRELUDE} (= {term} {e}) (refl {e})')

main()
