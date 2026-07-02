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
# monus — matches native i32 only while results stay >= 0), umul(23), upred(20). Comparisons too: ult(28)
# and ueq(25) reduce to user-Nat 1/0 by TWO-argument structural recursion (via helper funs that re-match
# the second operand — an explicit (f fid scrut extra) call picks a fresh scrutinee from a field, which the
# `(rec)` shorthand cannot, so both operands decrement together). omega2gamma already lowers >, >=, <=, !=
# to lt/eq/+/- , so those five heads (+ - * lt eq) cover the whole comparison surface, all kernel-evaluated.
# `-` is honest monus: a program whose subtraction would go negative has a different meaning than native
# i32 wrap, so the encoder BAILS (exit 2) on it, as it does on any value it cannot encode small (unary
# numerals hit delta's arena wall — guard at MAXV) or any construct outside the supported subset.
#
# LOOPS: a bounded state machine (mutually-recursive gamma defs: entry -> guard -> body -> back-to-guard,
# with an exit state) becomes a delta FUEL-STRUCTURED fold. The two loop-carried locals are packed into a
# user Pair (cid 4, projected by fst=42/snd=43) threaded as the single extra arg (the kernel is binary-only
# by design). loopfn(S f, p) hands (guard(p), Pair(f,p)) to a brancher that RE-EVALUATES the guard in the
# kernel and either iterates on body(p) or exits — so delta re-runs the real loop (guard + body + result),
# with fuel only a safe termination bound (the encoder abstract-executes the loop over the concrete initial
# literals to get a trip count and to bounds-check every intermediate value). A miscompiled body/guard makes
# the native exit unreachable by this evaluation and is REJECTED. Scope: exactly two loop-carried locals,
# body arithmetic in + - * < == ; general while-loops with data-dependent bounds, /, %, and >2 carried
# locals are later slices.
#
# UNTRUSTED, like prover.py: a bad encoding can only make certs that FAIL or mis-state the meaning;
# meaning-fidelity is independently pinned by the kernel diamond over the same translator output.
import sys

MAXV = 200  # arena guard: unary numerals beyond this blow delta's node budget (200 admits 5! = 120)

PRELUDE = (
    "(data 2 0 0 0) (data 3 1 1 0) "
    "(fun 20 2 (k 2)) (fun 20 3 (v 0)) "                # upred:  pred Z = Z ; pred (S k) = k
    "(fun 21 2 (y 0)) (fun 21 3 (k 3 (rec 0))) "        # uadd:   add(Z,y)=y ; add(S x,y)=S(add(x,y))
    "(fun 22 2 (y 0)) (fun 22 3 (f 20 (rec 0))) "       # usub:   sub(Z,a)=a ; sub(S k,a)=pred(sub(k,a))  [a-b via sub(b,a)]
    "(fun 23 2 (k 2)) (fun 23 3 (f 21 (y 0) (rec 0))) " # umul:   mul(Z,y)=Z ; mul(S x,y)=add(y,mul(x,y))
    "(fun 24 2 (k 3 (k 2))) (fun 24 3 (k 2)) "          # iszero: Z->1 ; S _->0
    "(fun 25 2 (f 24 (y 0))) (fun 25 3 (f 26 (y 0) (v 0))) "  # ueq:  eq(Z,b)=iszero b ; eq(S x,b)=eqs(b,x)
    "(fun 26 2 (k 2)) (fun 26 3 (f 25 (v 0) (y 0))) "   # eqs (scrut=b): eq(Z,x)=0 ; eq(S y,x)=eq(y,x)
    "(fun 27 2 (k 2)) (fun 27 3 (k 3 (k 2))) "          # pos:    Z->0 ; S _->1
    "(fun 28 2 (f 27 (y 0))) (fun 28 3 (f 29 (y 0) (v 0))) "  # ult:  lt(Z,b)=pos b ; lt(S x,b)=lts(b,x)
    "(fun 29 2 (k 2)) (fun 29 3 (f 28 (y 0) (v 0)))"    # lts (scrut=b): lt(Z,x)=0 ; lt(S y,x)=lt(x,y)
)

# extra funs the LOOP path needs on top of PRELUDE: a 2-tuple (Pair, cid 4) and its projections.
LOOP_EXTRA = (
    " (data 4 2 0 0) "                                  # Pair a b  (packs the two loop-carried locals)
    "(fun 42 4 (v 0)) (fun 43 4 (v 1))"                 # fst / snd
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
    if len(e) == 3 and e[0] in ('+', '*', '-', 'lt', 'eq'):
        (ta, va), (tb, vb) = ev(e[1], env), ev(e[2], env)
        if e[0] == '+':
            v = va + vb; t = f'(f 21 {ta} {tb})'
        elif e[0] == '*':
            v = va * vb; t = f'(f 23 {ta} {tb})'
        elif e[0] == 'lt':                                 # 1 if a<b else 0 (kernel-decided)
            v = 1 if va < vb else 0; t = f'(f 28 {ta} {tb})'
        elif e[0] == 'eq':                                 # 1 if a==b else 0 (kernel-decided)
            v = 1 if va == vb else 0; t = f'(f 25 {ta} {tb})'
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


# ---- loop path: gamma state machine -> delta fuel-fold -----------------------------------------
# Two separate views of a gamma expression, no value/term fusion (the loop body's locals vary each
# iteration, so its delta term is purely SYNTACTIC while its concrete value comes from abstract exec):
#   tr(e, env)  -> delta term          (env: name -> delta-term string)
#   val(e, env) -> concrete int        (env: name -> int; enforces monus>=0 and 0..MAXV, like ev)

def tr(e, env):
    if isinstance(e, str):
        if e.lstrip('-').isdigit():
            return unary(int(e))
        if e in env:
            return env[e]
        sys.exit(2)
    if len(e) == 3 and e[0] in ('+', '*', '-', 'lt', 'eq'):
        a, b = tr(e[1], env), tr(e[2], env)
        return {'+': f'(f 21 {a} {b})', '*': f'(f 23 {a} {b})', 'lt': f'(f 28 {a} {b})',
                'eq': f'(f 25 {a} {b})', '-': f'(f 22 {b} {a})'}[e[0]]
    sys.exit(2)

def val(e, env):
    if isinstance(e, str):
        if e.lstrip('-').isdigit():
            return int(e)
        if e in env:
            return env[e]
        sys.exit(2)
    if len(e) == 3 and e[0] in ('+', '*', '-', 'lt', 'eq'):
        a, b = val(e[1], env), val(e[2], env)
        if   e[0] == '+':  r = a + b
        elif e[0] == '*':  r = a * b
        elif e[0] == 'lt': r = 1 if a < b else 0
        elif e[0] == 'eq': r = 1 if a == b else 0
        else:
            if a < b:
                sys.exit(2)                                # monus would go negative
            r = a - b
        if r < 0 or r > MAXV:
            sys.exit(2)
        return r
    sys.exit(2)

def flatten_lets(node):
    """peel a (let n v body)* chain -> (bindings, innermost tail node)"""
    binds = {}
    while isinstance(node, list) and node and node[0] == 'let':
        _, n, v, node = node
        binds[n] = v
    return binds, node

def inline(e, binds):
    """substitute a def's local temps away, leaving only loop-param names + literals + operators"""
    if isinstance(e, str):
        return inline(binds[e], binds) if e in binds else e
    return [e[0]] + [inline(x, binds) for x in e[1:]]

def encode_loop(defs, call, claimed):
    dmap = {d[1]: d for d in defs}
    # entry: (let init...)* -> (guard i0 i1)
    entry = dmap.get(call[0])
    if entry is None:
        sys.exit(2)
    ebinds, etail = flatten_lets(entry[3])
    if not (isinstance(etail, list) and etail[0] in dmap):
        sys.exit(2)
    guard_name = etail[0]
    init = [val(inline(a, ebinds), {}) for a in etail[1:]]
    if len(init) != 2:
        sys.exit(2)                                        # scope: exactly two loop-carried locals
    # guard state: (if COND (body l0 l1) (exit l0 l1))
    gdef = dmap[guard_name]
    params = gdef[2]
    gbody = gdef[3]
    if len(params) != 2 or not (isinstance(gbody, list) and gbody[0] == 'if'):
        sys.exit(2)
    cond, then_call, else_call = gbody[1], gbody[2], gbody[3]
    body_name, exit_name = then_call[0], else_call[0]
    if body_name not in dmap or exit_name not in dmap:
        sys.exit(2)
    # body state: (let step...)* -> (guard n0 n1)
    bbinds, btail = flatten_lets(dmap[body_name][3])
    if not (isinstance(btail, list) and btail[0] == guard_name and len(btail) == 3):
        sys.exit(2)
    newl = [inline(a, bbinds) for a in btail[1:]]           # next loop-var exprs over params
    exitexpr = dmap[exit_name][3]                          # returned expression over params
    p0, p1 = params
    # abstract-execute over the concrete initial literals: trip count + bounds-check every step
    cur = {p0: init[0], p1: init[1]}
    trips = 0
    while val(cond, cur) == 1:
        cur = {p0: val(newl[0], cur), p1: val(newl[1], cur)}
        trips += 1
        if trips > MAXV:
            sys.exit(2)                                    # runaway / not a bounded loop in range
    val(exitexpr, cur)                                     # bounds-check the exit value too
    fuel = trips + 2                                       # safe over-estimate; guard decides termination
    # delta terms: p = (y 0) inside loopfn ; inside brancher the pair is snd of (y 0)=Pair(f,p)
    env_p = {p0: '(f 42 (y 0))',        p1: '(f 43 (y 0))'}
    env_s = {p0: '(f 42 (f 43 (y 0)))', p1: '(f 43 (f 43 (y 0)))'}
    prelude = (PRELUDE + LOOP_EXTRA + " "
        f"(fun 44 2 {tr(exitexpr, env_p)}) "                                  # fuel=Z  -> exit(p)  (unreached)
        f"(fun 44 3 (f 45 {tr(cond, env_p)} (k 4 (v 0) (y 0)))) "             # loopfn(S f,p)=brancher(guard p, Pair(f,p))
        f"(fun 45 2 {tr(exitexpr, env_s)}) "                                  # guard false -> exit(snd fp)
        f"(fun 45 3 (f 44 (f 42 (y 0)) (k 4 {tr(newl[0], env_s)} {tr(newl[1], env_s)})))")  # guard true -> loopfn(f, body p)
    term = f'(f 44 {unary(fuel)} (k 4 {unary(init[0])} {unary(init[1])}))'
    e = unary(claimed)
    print(f'{prelude} (= {term} {e}) (refl {e})')

def main():
    claimed = int(sys.argv[1])
    forms = parse_all(sys.stdin.read())
    defs = [f for f in forms if isinstance(f, list) and f and f[0] == 'def']
    call = forms[-1]
    if not isinstance(call, list):
        sys.exit(2)
    if len(defs) >= 3:                                     # mutually-recursive state machine -> loop path
        encode_loop(defs, call, claimed)
        return
    if len(defs) != 1:
        sys.exit(2)                                        # multiple machines (cross-call) -> later slices
    _, name, params, body = defs[0]
    args = call[1:]
    if call[0] != name or len(args) != len(params):
        sys.exit(2)
    env = {p: ev(a, {}) for p, a in zip(params, args)}
    term, _ = ev(body, env)
    e = unary(claimed)
    print(f'{PRELUDE} (= {term} {e}) (refl {e})')

main()
