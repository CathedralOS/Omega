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
# DIV / MOD: `/` and `%` are kernel-EVALUATED by self-fueled repeated monus (udiv/umod; the dividend is its
# own termination bound). Fully faithful — delta re-subtracts — but each op costs O(quotient) reduction
# steps, and the checker's recursive normalizer runs out of native stack past ~6 steps, so the encoder
# guards the quotient at SAFE_Q and BAILS above it (arena/stack wall, like MAXV). A future slice can lift
# this by switching to a quotient-WITNESS check (a == b*q + r  &&  r < b — shallow, any size) at the cost of
# the encoder supplying q. div-by-zero also bails.
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
# CROSS-MACHINE CALLS: a program may define value-returning FREE machines (`machine f(x: i32) -> i32 {..}`)
# that Main::main and each other call. omega2gamma emits them as sibling gamma defs `m{i}_me` with calls
# `(m{i}_me args)`. The encoder INLINES each call — bind the callee's params to the (encoded) args and encode
# its body — so delta recomputes the whole nested computation; the entry is the def named in the final call.
# Inlining is depth-bounded (MAX_INLINE) so the term stays in the arena; terminating recursion inlines
# finitely, deeper ones bail. (Loops inside a called machine / recursive machines past the bound are later.)
#
# UNTRUSTED, like prover.py: a bad encoding can only make certs that FAIL or mis-state the meaning;
# meaning-fidelity is independently pinned by the kernel diamond over the same translator output.
import sys

MAXV = 200  # arena guard: unary numerals beyond this blow delta's node budget (200 admits 5! = 120)
MAX_INLINE = 64  # cross-machine calls inline their callee; bound the nesting so terms stay in the arena
DEFS = {}        # machine name -> (params, body); set in main, read by ev for cross-machine call inlining
DEPTH = [0]      # current inline depth (list so nested ev calls can mutate it)

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

# a user 2-tuple (Pair, cid 4) + projections — used to pack loop locals AND div/mod's (dividend,divisor).
PAIR_DEFS = (
    " (data 4 2 0 0) "
    "(fun 42 4 (v 0)) (fun 43 4 (v 1))"                 # fst / snd
)

# div / mod as SELF-FUELED fuel-folds (the dividend is its own termination bound: each step subtracts the
# divisor, at most `a` steps). udiv(a,b) = floor(a/b) ; umod(a,b) = a mod b. Both fully kernel-EVALUATED.
# Cost is O(quotient) reduction steps, so the encoder guards the quotient at SAFE_Q (arena wall, like MAXV).
DIV_DEFS = (
    " (fun 46 2 (k 2)) (fun 46 3 (f 47 (k 3 (v 0)) (k 4 (k 3 (v 0)) (y 0)))) "                     # udiv
    "(fun 47 2 (k 2)) (fun 47 3 (f 48 (f 28 (f 42 (y 0)) (f 43 (y 0))) (k 4 (v 0) (y 0)))) "       # divf(fuel,Pair(a,b))
    "(fun 48 3 (k 2)) "                                                                            #   a<b -> 0
    "(fun 48 2 (k 3 (f 47 (f 42 (y 0)) (k 4 (f 22 (f 43 (f 43 (y 0))) (f 42 (f 43 (y 0)))) (f 43 (f 43 (y 0))))))) "  # a>=b -> S(divf(f,Pair(a-b,b)))
    "(fun 49 2 (k 2)) (fun 49 3 (f 50 (k 3 (v 0)) (k 4 (k 3 (v 0)) (y 0)))) "                      # umod
    "(fun 50 2 (f 42 (y 0))) (fun 50 3 (f 51 (f 28 (f 42 (y 0)) (f 43 (y 0))) (k 4 (v 0) (y 0)))) "# modf(fuel,Pair(a,b))
    "(fun 51 3 (f 42 (f 43 (y 0)))) "                                                              #   a<b -> a
    "(fun 51 2 (f 50 (f 42 (y 0)) (k 4 (f 22 (f 43 (f 43 (y 0))) (f 42 (f 43 (y 0)))) (f 43 (f 43 (y 0))))))"  # a>=b -> modf(f,Pair(a-b,b))
)

# everything reusable in one base; the loop path adds only its two per-loop funs (loopfn/brancher).
BASE = PRELUDE + PAIR_DEFS + DIV_DEFS

SAFE_Q = 6   # div/mod quotient guard: > this many subtraction steps overflows the checker's node budget

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
    if len(e) == 3 and e[0] in ('+', '*', '-', 'lt', 'eq', '/', '%'):
        (ta, va), (tb, vb) = ev(e[1], env), ev(e[2], env)
        if e[0] == '+':
            v = va + vb; t = f'(f 21 {ta} {tb})'
        elif e[0] == '*':
            v = va * vb; t = f'(f 23 {ta} {tb})'
        elif e[0] == 'lt':                                 # 1 if a<b else 0 (kernel-decided)
            v = 1 if va < vb else 0; t = f'(f 28 {ta} {tb})'
        elif e[0] == 'eq':                                 # 1 if a==b else 0 (kernel-decided)
            v = 1 if va == vb else 0; t = f'(f 25 {ta} {tb})'
        elif e[0] in ('/', '%'):                           # kernel-evaluated by repeated monus
            if vb == 0 or va // vb > SAFE_Q:
                sys.exit(2)                                # div-by-zero / quotient past the arena wall
            if e[0] == '/':
                v = va // vb; t = f'(f 46 {ta} {tb})'
            else:
                v = va % vb;  t = f'(f 49 {ta} {tb})'
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
    if isinstance(e, list) and e and isinstance(e[0], str) and e[0] in DEFS:
        params, body = DEFS[e[0]]                          # cross-machine call: inline the callee's body
        args = e[1:]
        if len(args) != len(params):
            sys.exit(2)
        DEPTH[0] += 1
        if DEPTH[0] > MAX_INLINE:
            sys.exit(2)                                    # runaway / too-deep to keep the term in the arena
        env2 = {p: ev(a, env) for p, a in zip(params, args)}
        r = ev(body, env2)
        DEPTH[0] -= 1
        return r
    sys.exit(2)                                            # if/match, unresolved name, ... -> later slices


# ---- loop path: gamma state machine -> delta encoding ------------------------------------------
# Two separate views of a gamma expression (the loop body's locals vary each iteration):
#   tr(e, env)  -> delta term          (env: name -> delta-term string)
#   val(e, env) -> concrete int        (env: name -> int; enforces monus>=0, div/quotient bound, 0..MAXV)
# Both cover the same operators as straight-line ev, INCLUDING / and %. div/mod are used only on the
# UNROLL path (below): each op is then a standalone recursive udiv/umod on that step's operands — NOT
# nested in a fuel recursion — so the depth stays bounded (val guards the per-step quotient at SAFE_Q).

def tr(e, env):
    if isinstance(e, str):
        if e.lstrip('-').isdigit():
            return unary(int(e))
        if e in env:
            return env[e]
        sys.exit(2)
    if len(e) == 3 and e[0] in ('+', '*', '-', 'lt', 'eq', '/', '%'):
        a, b = tr(e[1], env), tr(e[2], env)
        return {'+': f'(f 21 {a} {b})', '*': f'(f 23 {a} {b})', 'lt': f'(f 28 {a} {b})',
                'eq': f'(f 25 {a} {b})', '-': f'(f 22 {b} {a})',
                '/': f'(f 46 {a} {b})', '%': f'(f 49 {a} {b})'}[e[0]]
    sys.exit(2)

def val(e, env):
    if isinstance(e, str):
        if e.lstrip('-').isdigit():
            return int(e)
        if e in env:
            return env[e]
        sys.exit(2)
    if len(e) == 3 and e[0] in ('+', '*', '-', 'lt', 'eq', '/', '%'):
        a, b = val(e[1], env), val(e[2], env)
        if   e[0] == '+':  r = a + b
        elif e[0] == '*':  r = a * b
        elif e[0] == 'lt': r = 1 if a < b else 0
        elif e[0] == 'eq': r = 1 if a == b else 0
        elif e[0] in ('/', '%'):
            if b == 0 or a // b > SAFE_Q:
                sys.exit(2)                                # div-by-zero / per-step quotient past the wall
            r = a // b if e[0] == '/' else a % b
        else:
            if a < b:
                sys.exit(2)                                # monus would go negative
            r = a - b
        if r < 0 or r > MAXV:
            sys.exit(2)
        return r
    sys.exit(2)

def uses_divmod(e):
    return isinstance(e, list) and (e[0] in ('/', '%') or any(uses_divmod(x) for x in e[1:]))

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

MAXLOCALS = 4   # loop-carried locals: nested-Pair packing depth stays small enough for the arena
UNROLL_MAX = 12 # div/mod-bearing loops are unrolled; cap iterations so the exit term stays shallow
MODNODE_MAX = 4 # ... and cap div/mod nodes in the exit term: each nests a recursive udiv/umod, and past
                # ~4 the composed reduction depth overflows the checker's stack (Fibonacci-worst gcds bail)

# N loop-carried locals pack right-nested: P = Pair(l0, Pair(l1, ..., Pair(l_{N-2}, l_{N-1}))).
def pack(terms):
    t = terms[-1]
    for x in reversed(terms[:-1]):
        t = f'(k 4 {x} {t})'
    return t

def proj(k, n, base):                     # delta term selecting local k out of an n-tuple rooted at `base`
    t = base
    for _ in range(k):
        t = f'(f 43 {t})'                 # snd
    return t if k == n - 1 else f'(f 42 {t})'   # innermost is snd^{n-1}; earlier ones take fst

def encode_loop(defs, call, claimed):
    dmap = {d[1]: d for d in defs}
    # entry: (let init...)* -> (guard i0 .. i_{n-1})
    entry = dmap.get(call[0])
    if entry is None:
        sys.exit(2)
    ebinds, etail = flatten_lets(entry[3])
    if not (isinstance(etail, list) and etail[0] in dmap):
        sys.exit(2)
    guard_name = etail[0]
    init = [val(inline(a, ebinds), {}) for a in etail[1:]]
    n = len(init)
    if n < 1 or n > MAXLOCALS:
        sys.exit(2)
    # guard state: (if COND (then l..) (else l..)) — either branch may be the body; the one that
    # tail-calls back to the guard IS the body (loop), the other is the exit. This handles both
    # `while COND` (then=body) and `while !COND` / exit-when-true (else=body, e.g. gcd's b==0).
    gdef = dmap[guard_name]
    params = gdef[2]
    gbody = gdef[3]
    if len(params) != n or not (isinstance(gbody, list) and gbody[0] == 'if'):
        sys.exit(2)
    cond, then_name, else_name = gbody[1], gbody[2][0], gbody[3][0]
    if then_name not in dmap or else_name not in dmap:
        sys.exit(2)
    def loops_back(nm):
        _, t = flatten_lets(dmap[nm][3])
        return isinstance(t, list) and len(t) == n + 1 and t[0] == guard_name
    tb, eb = loops_back(then_name), loops_back(else_name)
    if tb and not eb:
        body_name, exit_name, cont = then_name, else_name, cond        # loop while COND is 1
    elif eb and not tb:
        body_name, exit_name, cont = else_name, then_name, ['eq', cond, '0']  # loop while COND is 0
    else:
        sys.exit(2)                                        # not a single-exit loop
    # body state: (let step...)* -> (guard n0 .. n_{n-1})
    bbinds, btail = flatten_lets(dmap[body_name][3])
    newl = [inline(a, bbinds) for a in btail[1:]]           # next loop-var exprs over params
    exitexpr = dmap[exit_name][3]                          # returned expression over params

    # A body containing div/mod can't ride the fuel-fold: a recursive udiv/umod nested inside the fuel
    # recursion overflows the checker's stack. Instead UNROLL — the trip count is data-dependent but the
    # encoder knows it, so symbolically step the loop, threading each local as a delta term; each div/mod
    # is then a standalone (shallow) op. The meaning is the exit expression over the final terms. This is
    # result-level TV (delta recomputes the arithmetic of the taken path; the trip count is the encoder's
    # witness, as with the fuel bound). Used for gcd and other small data-dependent loops.
    if any(uses_divmod(x) for x in newl + [exitexpr, cont]):
        tenv = {params[k]: unary(init[k]) for k in range(n)}   # term env  (delta terms)
        venv = {params[k]: init[k] for k in range(n)}          # value env (concrete)
        steps = 0
        while val(cont, venv) == 1:
            if steps >= UNROLL_MAX:
                sys.exit(2)                                    # too many iterations to unroll safely
            tenv = {params[k]: tr(newl[k], tenv) for k in range(n)}
            venv = {params[k]: val(newl[k], venv) for k in range(n)}
            steps += 1
        meaning = tr(exitexpr, tenv)
        if meaning.count('(f 46') + meaning.count('(f 49') > MODNODE_MAX:
            sys.exit(2)                                        # exit term too deep -> would crash the checker
        e = unary(claimed)
        print(f'{BASE} (= {meaning} {e}) (refl {e})')
        return

    # abstract-execute over the concrete initial literals: trip count + bounds-check every step
    cur = {params[k]: init[k] for k in range(n)}
    trips = 0
    while val(cont, cur) == 1:
        cur = {params[k]: val(newl[k], cur) for k in range(n)}
        trips += 1
        if trips > MAXV:
            sys.exit(2)                                    # runaway / not a bounded loop in range
    val(exitexpr, cur)                                     # bounds-check the exit value too
    fuel = trips + 2                                       # safe over-estimate; guard decides termination
    # delta terms: inside loopfn the tuple is p=(y 0) ; inside brancher it is snd of (y 0)=Pair(f,p)
    env_p = {params[k]: proj(k, n, '(y 0)')        for k in range(n)}
    env_s = {params[k]: proj(k, n, '(f 43 (y 0))') for k in range(n)}
    newpack = pack([tr(newl[k], env_s) for k in range(n)])
    prelude = (BASE + " "
        f"(fun 44 2 {tr(exitexpr, env_p)}) "                             # fuel=Z  -> exit(p)  (unreached)
        f"(fun 44 3 (f 45 {tr(cont, env_p)} (k 4 (v 0) (y 0)))) "        # loopfn(S f,p)=brancher(continue? p, Pair(f,p))
        f"(fun 45 2 {tr(exitexpr, env_s)}) "                            # guard false -> exit(snd fp)
        f"(fun 45 3 (f 44 (f 42 (y 0)) {newpack}))")                     # guard true -> loopfn(f, body p)
    term = f'(f 44 {unary(fuel)} {pack([unary(v) for v in init])})'
    e = unary(claimed)
    print(f'{prelude} (= {term} {e}) (refl {e})')

def main():
    global DEFS
    claimed = int(sys.argv[1])
    forms = parse_all(sys.stdin.read())
    defs = [f for f in forms if isinstance(f, list) and f and f[0] == 'def']
    call = forms[-1]
    if not isinstance(call, list):
        sys.exit(2)
    DEFS = {d[1]: (d[2], d[3]) for d in defs}
    entry = call[0]                                       # the def called at the very end is the entry
    if entry not in DEFS:
        sys.exit(2)
    prefix = entry[:-3] if entry.endswith('_me') else entry
    # a state-machine LOOP has the entry's own state defs (m{i}_s0, ...); route those to the loop encoder.
    if any(n.startswith(prefix + '_s') for n in DEFS):
        encode_loop(defs, call, claimed)
        return
    # otherwise: straight-line, possibly calling value-returning FREE machines -> inline the calls (ev).
    params, body = DEFS[entry]
    args = call[1:]
    if len(args) != len(params):
        sys.exit(2)
    env = {p: ev(a, {}) for p, a in zip(params, args)}
    term, _ = ev(body, env)
    e = unary(claimed)
    print(f'{BASE} (= {term} {e}) (refl {e})')

main()
