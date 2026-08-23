#!/usr/bin/env python3
# gamma2claim.py — UNTRUSTED encoder for meaning-route translation validation at the SUMMIT rung.
#
# Reads an omega2gamma-translated gamma program (defs + a final expression, fully CLOSED — Omega samples
# take no runtime input) on stdin and abstract-executes it, building the program's meaning as an UNFOLDED
# kernel arithmetic term: every `+` in the computation becomes a `(p A B)` node over unary numerals, so the
# claim
#       (= <meaning term> <unary exit>)   (refl <unary exit>)
# is accepted by proof-kernel/check.beta only if the kernel's own CONVERSION re-computes the entire arithmetic of
# the sample and reaches the same exit. Control (if / match arms, call targets) is decided by the encoder —
# the same trust shape as tv-encode.py's unrolled loops: a bad decision mis-states the meaning and the claim
# simply fails against the independently-run interpreter exit. Scope (grown far past slice 1): the full
# arithmetic/comparison/logic set incl. shifts-as-mul/div, arrays and record arrays (Cons spines / Pair
# tuples), slices (nth/drop/take/len), case data (tags + Pair payloads), value calls and machine tail
# calls, strings, and dual-channel (Pair exit stdout) results. Anything outside REFUSES (exit 2).
#
# OBLIGATIONS (lines 3+, all kernel-checked): division safety, array bounds, arithmetic witnesses (value
# pins + literal certificates inside the measured reduction envelope), domain-erasure no-underflow
# witnesses, and boundary-range byte witnesses for every value crossing the process boundary.
#
# STRUCTURAL RESULTS: a sample whose final value is a constructor tree (e.g. cli_mvp's output char list)
# gets a structural claim instead — left side the tree with each leaf's COMPUTED term, right side the
# literal tree, fresh (data CID ..) decls per constructor — so the kernel re-computes every leaf and the
# tree shape. The concrete render (interp.beta's exact print format) is emitted as a final `#render` line
# the gate string-compares against the interpreter's stdout: the structure is PINNED to the real run, not
# just the exit code (which is always 0 for constructor-valued programs).
#
# VALUE MODES (chosen per sample by retry): kernel-native p/m over s/z nats (+/* only) -> user nats +
# tv-encode's fun prelude (-,/,% appear) -> ℤ difference pairs (a subtraction underflowed: Under) ->
# BINARY bit-spines (an intermediate exceeded the unary wall: Big). Heavy ops are WITNESSED (value-pin
# certs + literal certs proving the defining property) so every certificate stays inside the measured
# reduction envelope of the 64 MiB alpha image.
#
# stdout line 1: `<computed exit> <claim cert>`; line 2: the off-by-one NEGATIVE-control cert the kernel
# must reject. The gate cross-checks the exit against both the interpreter run and the documented intent.
import sys
sys.setrecursionlimit(200000)          # deep dispatch chains in translated samples

FUEL = 500000
MAXV = 20000                           # hard unary wall (zpair mode: difference-pair components)
UWALL = 500                            # unary-mode PREFERENCE wall: beyond it a sample re-encodes in
                                       # binary, whose certs stay decidable by the tightest diamond leg
                                       # (checker.gamma-on-interp dies on ~2000-deep unary walks)
WALL = [MAXV]                          # the active wall, set per mode by run()

# tv-encode.py's kernel-side user-nat machinery, verbatim (uadd/usub/umul/ueq/ult + fueled udiv/umod):
# engaged only when the sample uses - / %, so the +/* samples keep their kernel-native p/m forms.
USER_PRELUDE = (
    "(data 2 0 0 0) (data 3 1 1 0) "
    "(fun 20 2 (k 2)) (fun 20 3 (v 0)) "
    "(fun 21 2 (y 0)) (fun 21 3 (k 3 (rec 0))) "
    "(fun 22 2 (y 0)) (fun 22 3 (f 20 (rec 0))) "
    "(fun 23 2 (k 2)) (fun 23 3 (f 21 (y 0) (rec 0))) "
    "(fun 24 2 (k 3 (k 2))) (fun 24 3 (k 2)) "
    "(fun 25 2 (f 24 (y 0))) (fun 25 3 (f 26 (y 0) (v 0))) "
    "(fun 26 2 (k 2)) (fun 26 3 (f 25 (v 0) (y 0))) "
    "(fun 27 2 (k 2)) (fun 27 3 (k 3 (k 2))) "
    "(fun 28 2 (f 27 (y 0))) (fun 28 3 (f 29 (y 0) (v 0))) "
    "(fun 29 2 (k 2)) (fun 29 3 (f 28 (y 0) (v 0)))"
    " (data 4 2 0 0) "
    "(fun 42 4 (v 0)) (fun 43 4 (v 1))"
    " (fun 46 2 (k 2)) (fun 46 3 (f 47 (k 3 (v 0)) (k 4 (k 3 (v 0)) (y 0)))) "
    "(fun 47 2 (k 2)) (fun 47 3 (f 48 (f 28 (f 42 (y 0)) (f 43 (y 0))) (k 4 (v 0) (y 0)))) "
    "(fun 48 3 (k 2)) "
    "(fun 48 2 (k 3 (f 47 (f 42 (y 0)) (k 4 (f 22 (f 43 (f 43 (y 0))) (f 42 (f 43 (y 0)))) (f 43 (f 43 (y 0))))))) "
    "(fun 49 2 (k 2)) (fun 49 3 (f 50 (k 3 (v 0)) (k 4 (k 3 (v 0)) (y 0)))) "
    "(fun 50 2 (f 42 (y 0))) (fun 50 3 (f 51 (f 28 (f 42 (y 0)) (f 43 (y 0))) (k 4 (v 0) (y 0)))) "
    "(fun 51 3 (f 42 (f 43 (y 0)))) "
    "(fun 51 2 (f 50 (f 42 (y 0)) (k 4 (f 22 (f 43 (f 43 (y 0))) (f 42 (f 43 (y 0)))) (f 43 (f 43 (y 0))))))"
)


# BINARY NUMERALS (the compound-values opener): samples whose intermediates exceed the unary wall re-encode
# with little-endian bit-spine values — (k 70) zero, (k 71 x) = 2x, (k 72 x) = 2x+1, CANONICAL (the most
# significant bit is always B1, zero is bare BNIL). Addition is carry-passing mutual recursion (the ult
# swap trick peels both operands: badd0/badd1 dispatch the first arg's bit, then the aNrM helpers dispatch
# the second's with the first's tail remembered); multiplication is shift-and-add. Every unfold run is
# O(bits), so 72-million LCG states cost ~27 frames — exponentially inside the reduction envelope.
BIN_PRELUDE = (
    "(data 70 0 0 0) (data 71 1 1 0) (data 72 1 1 0) "
    "(fun 80 70 (k 72 (k 70))) (fun 80 71 (k 72 (v 0))) (fun 80 72 (k 71 (f 80 (v 0)))) "   # inc
    "(fun 81 70 (y 0)) (fun 81 71 (f 82 (y 0) (v 0))) (fun 81 72 (f 83 (y 0) (v 0))) "      # badd0
    "(fun 82 70 (k 71 (y 0))) (fun 82 71 (k 71 (f 81 (v 0) (y 0)))) "                       # x-bit 0
    "(fun 82 72 (k 72 (f 81 (v 0) (y 0)))) "
    "(fun 83 70 (k 72 (y 0))) (fun 83 71 (k 72 (f 81 (v 0) (y 0)))) "                       # x-bit 1
    "(fun 83 72 (k 71 (f 84 (v 0) (y 0)))) "
    "(fun 84 70 (f 80 (y 0))) (fun 84 71 (f 85 (y 0) (v 0))) (fun 84 72 (f 86 (y 0) (v 0))) "  # badd1
    "(fun 85 70 (k 72 (y 0))) (fun 85 71 (k 72 (f 81 (v 0) (y 0)))) "                       # carry, bit 0
    "(fun 85 72 (k 71 (f 84 (v 0) (y 0)))) "
    "(fun 86 70 (k 71 (f 80 (y 0)))) (fun 86 71 (k 71 (f 84 (v 0) (y 0)))) "                # carry, bit 1
    "(fun 86 72 (k 72 (f 84 (v 0) (y 0)))) "
    "(fun 87 70 (k 70)) (fun 87 71 (f 87 (v 0) (k 71 (y 0)))) "                             # bmul
    "(fun 87 72 (f 81 (y 0) (f 87 (v 0) (k 71 (y 0)))))"
)

BIGMAX = 10 ** 12                      # sanity wall for binary mode (40 bits)


def bin_lit(k):                        # canonical little-endian bit spine
    if k < 0:
        raise Out('negative value reached the binary encoder')
    if k > BIGMAX:
        raise Out('value %d exceeds the binary wall' % k)
    if k == 0:
        return '(k 70)'
    return '(k %d %s)' % (72 if k & 1 else 71, bin_lit(k >> 1))


class Out(Exception):
    pass


class Big(Exception):                  # an intermediate exceeded the unary wall: retry in binary mode
    pass


def parse_all(src):
    toks = src.replace('(', ' ( ').replace(')', ' ) ').split()
    pos = [0]

    def rd():
        t = toks[pos[0]]; pos[0] += 1
        if t != '(':
            return int(t) if t.lstrip('-').isdigit() else t
        o = []
        while toks[pos[0]] != ')':
            o.append(rd())
        pos[0] += 1
        return o

    forms = []
    while pos[0] < len(toks):
        forms.append(rd())
    return forms


def nat(k):
    if k < 0:
        raise Out('negative value reached the term encoder')
    if k > WALL[0]:
        raise Big()
    return 'z' if k == 0 else '(s %s)' % nat(k - 1)


def unat(k):                           # user-nat literal (k 3 (k 3 ... (k 2)))
    if k < 0:
        raise Out('negative value reached the term encoder')
    if k > WALL[0]:
        raise Big()
    return '(k 2)' if k == 0 else '(k 3 %s)' % unat(k - 1)


class V:                                   # a value: concrete int `n` + the TERM tree(s) that compute it.
    __slots__ = ('n', 't', 'nt')           # zpair mode: n is the ℤ value, (t, nt) the (pos, neg) components

    def __init__(self, n, t, nt=None):
        self.n = n
        self.t = t
        self.nt = nt


class Under(Exception):                    # an underflowing subtraction: retry the sample in zpair mode
    pass


def main():
    src = sys.stdin.read()
    try:
        try:
            run(src, zpair=False)
        except Under:
            run(src, zpair=True)           # an underflow: re-encode with ℤ difference-pair values
    except Big:
        run(src, zpair=False, binary=True)  # an intermediate above the unary wall: binary bit-spines


def run(src, zpair, binary=False):
    forms = parse_all(src)
    # USER mode when -,/,% appear: values ride user nats and ops become kernel user-fun applications.
    # ZPAIR mode (an underflowing subtraction was hit): every value is a (pos, neg) pair of user nats —
    # componentwise uadd for +, swapped for -, cross terms for * — and the claim P = uadd(exit, N) makes the
    # kernel verify pos - neg = exit in ℤ, with no negative ever materializing (the refinement pillar's
    # difference-pair move, replayed kernel-side).
    user = zpair or any(('(%s ' % op) in src for op in ('-', '/', '%'))
    lit = bin_lit if binary else (unat if user else nat)
    WALL[0] = MAXV if zpair else UWALL     # zpair keeps the hard wall; plain unary defers to binary early
    Z0 = '(k 2)'
    defs = {}
    top = None
    for f in forms:
        if isinstance(f, list) and f and f[0] == 'def':
            defs[f[1]] = (f[2], f[3])
        else:
            top = f
    if top is None:
        raise Out('no top-level expression')
    fuel = [FUEL]
    vcs = []                               # SAFETY OBLIGATIONS: one kernel-checked claim per / and % site —
                                           # iszero(divisor) reduces to 0, i.e. the kernel re-computes the
                                           # divisor and confirms the division cannot trap (omega's
                                           # obligations.rs concept, discharged by the lattice's own anchor)
    ctors = {}                             # constructor name -> (cid, arity): fresh ids for structural claims

    def ctor_cid(name, arity):
        if name in ctors:
            cid, ar = ctors[name]
            if ar != arity:
                raise Out('constructor %s used at two arities' % name)
            return cid
        cid = 100 + len(ctors)
        ctors[name] = (cid, arity)
        return cid

    def num(x):                            # constructor values must never reach a numeric position: refuse
        if not isinstance(x, V):
            raise Out('constructor value in a numeric position')
        return x

    # THE REDUCTION ENVELOPE (measured, not assumed): check.beta runs in the alpha VM's fixed 64 MiB image;
    # a user-fun unfold RUN of depth ~2000+ (uadd recursing on its first argument's value) or ~3000 total
    # per certificate collides stack and arena — the old quotient wall was this limit wearing a disguise.
    # Heavy arithmetic is therefore split across CERTIFICATES (each check.exe run has fresh memory), the
    # multi-lemma assembly precedent applied to arithmetic:
    #   value-pin cert   (= <operand term> <literal>)      — walk-only; kernel re-computes the expression
    #   literal certs    chunked additions, first argument <= CHUNK, proving the op's arithmetic
    # and the op's result term becomes the literal, so downstream claims stay inside the envelope.
    CHUNK = 400                            # matched to the TIGHTEST diamond leg (checker.gamma-on-interp
    DIRECT_MUL = 400                       # ~400 unfolds; check.beta alone handles ~2000) so every cert
                                           # is decidable by ALL three checkers, not just the anchor

    def boundary_vc(n):                    # BOUNDARY-RANGE OBLIGATION (omega boundary.rs's concept):
        if n < 0 or n > 255:               # a value crossing the process boundary (exit code, stdout
            raise Out('boundary value %d outside the byte range' % n)   # byte) must BE a byte. The
        d = 255 - n                        # kernel checks n + (255-n) = 255 — an addition witness that
        if binary:                         # works uniformly in every mode (kernel p, user f 21, bin f 81)
            vcs.append('(= (f 81 %s %s) %s) (refl %s)'
                       % (bin_lit(min(n, d)), bin_lit(max(n, d)), bin_lit(255), bin_lit(255)))
        elif user:
            vcs.append('(= (f 21 %s %s) %s) (refl %s)'
                       % (unat(min(n, d)), unat(max(n, d)), unat(255), unat(255)))
        else:
            vcs.append('(= (p %s %s) %s) (refl %s)' % (nat(n), nat(d), nat(255), nat(255)))

    def pin(v):                            # kernel re-computes v's expression: (= v.t literal)
        litv = lit(v.n)
        if v.t != litv:
            vcs.append('(= %s %s) (refl %s)' % (v.t, litv, litv))

    def add_cert(x, y):                    # one in-envelope literal addition cert; returns the sum
        s = x + y
        if s > WALL[0]:
            raise Out('witness sum %d exceeds the unary wall' % s)
        lo, hi = (x, y) if x <= y else (y, x)
        if lo > CHUNK:
            raise Out('witness addend %d exceeds the chunk envelope' % lo)
        vcs.append('(= (f 21 %s %s) %s) (refl %s)' % (unat(lo), unat(hi), unat(s), unat(s)))
        return s

    def chunks(v):
        out = [CHUNK] * (v // CHUNK)
        if v % CHUNK:
            out.append(v % CHUNK)
        return out

    def wit_sum(parts, start=0):           # chunked chain: start + sum(parts), one cert per addition
        acc = start
        for p in parts:
            acc = add_cert(p, acc)
        return acc

    def wit_mul(u, v):                     # literal certs proving u*v; returns the product
        if u == 0 or v == 0:
            return 0
        cnt, big = (u, v) if u <= v else (v, u)
        if cnt * big <= DIRECT_MUL:
            prod = cnt * big
            vcs.append('(= (f 23 %s %s) %s) (refl %s)' % (unat(cnt), unat(big), unat(prod), unat(prod)))
            return prod
        return wit_sum(chunks(big) * cnt)

    # ARRAY-BOUNDS OBLIGATIONS: omega2gamma lowers arrays to Cons spines walked by its `nth`/`setl` helpers,
    # whose Nil arms return a SILENT default on overrun (0 / Nil) — the exact silent-OOB shape obligations
    # exist to forbid. At every user-level nth/setl call the kernel re-computes the INDEX EXPRESSION and
    # confirms it lands inside the spine: ult(idx, len) = 1 (zpair: neg <= pos and pos < len + neg, the
    # difference-pair reading of 0 <= idx < len). Recursive inner calls are the helper's own walk, not user
    # accesses — suppressed via inarr.
    inarr = [0]

    def spine_len(v):
        n = 0
        while isinstance(v, tuple) and v[0] == 'Cons' and len(v) == 3:
            n += 1
            v = v[2]
        return n if v == 'Nil' else None

    def arr_vc(lst, idx):
        n = spine_len(lst)
        if n is None or not isinstance(idx, V):
            raise Out('array access over a non-spine list or non-numeric index')
        if not user:
            raise Out('array access outside user mode')   # unreachable: nth/setl bodies contain `-`
        if binary:                          # idx + (len-idx) = len with a positive addend: idx < len
            if idx.n >= n:
                raise Out('index %d outside spine length %d' % (idx.n, n))
            pin(idx)
            vcs.append('(= (f 81 %s %s) %s) (refl %s)'
                       % (bin_lit(idx.n), bin_lit(n - idx.n), bin_lit(n), bin_lit(n)))
        elif zpair:
            vcs.append('(= (f 28 %s %s) (k 2)) (refl (k 2))' % (idx.t, idx.nt))
            vcs.append('(= (f 28 %s (f 21 %s %s)) (k 3 (k 2))) (refl (k 3 (k 2)))' % (idx.t, unat(n), idx.nt))
        else:
            vcs.append('(= (f 28 %s %s) (k 3 (k 2))) (refl (k 3 (k 2)))' % (idx.t, unat(n)))

    def ev(e, env):
        fuel[0] -= 1
        if fuel[0] <= 0:
            raise Out('fuel exhausted')
        if isinstance(e, int):
            return V(e, lit(e), Z0 if zpair else None)
        if isinstance(e, str):
            if e in env:
                return env[e]
            if e[0].isupper():
                return e                    # a bare constructor atom (Nil etc.) IS a value, not a variable
            return V(0, lit(0), Z0 if zpair else None)   # interp.beta's env_lookup: 0 on a miss — mirror it
                                            # (omega2gamma emits at least one unbound reference in the wild)
        h = e[0]
        if h == '+':
            a, b = num(ev(e[1], env)), num(ev(e[2], env))
            if abs(a.n) + abs(b.n) > WALL[0] and not binary:
                raise Big()
            if zpair:
                return V(a.n + b.n, '(f 21 %s %s)' % (a.t, b.t), '(f 21 %s %s)' % (a.nt, b.nt))
            if binary:
                lo, hi = (a, b) if a.n <= b.n else (b, a)
                return V(a.n + b.n, '(f 81 %s %s)' % (lo.t, hi.t))
            if not user:
                return V(a.n + b.n, '(p %s %s)' % (a.t, b.t))
            lo, hi = (a, b) if a.n <= b.n else (b, a)      # uadd unfolds its FIRST arg's value: orient small
            if lo.n > CHUNK:                                # both heavy: witness across certs
                pin(a), pin(b)
                s = wit_sum(chunks(lo.n), hi.n)
                return V(s, unat(s))
            return V(a.n + b.n, '(f 21 %s %s)' % (lo.t, hi.t))
        if h == '*':
            a, b = num(ev(e[1], env)), num(ev(e[2], env))
            if abs(a.n * b.n) > WALL[0] and not binary:
                raise Big()
            if zpair:                       # (p1-n1)(p2-n2) = (p1p2+n1n2) - (p1n2+n1p2)
                return V(a.n * b.n,
                         '(f 21 (f 23 %s %s) (f 23 %s %s))' % (a.t, b.t, a.nt, b.nt),
                         '(f 21 (f 23 %s %s) (f 23 %s %s))' % (a.t, b.nt, a.nt, b.t))
            if binary:
                if a.n == 0 or b.n == 0:
                    pin(a), pin(b)         # a zero factor would shift junk bits: pin and take the literal
                    return V(0, '(k 70)')
                lo, hi = (a, b) if a.n <= b.n else (b, a)
                return V(a.n * b.n, '(f 87 %s %s)' % (lo.t, hi.t))
            if not user:
                return V(a.n * b.n, '(m %s %s)' % (a.t, b.t))
            if a.n * b.n <= DIRECT_MUL:
                lo, hi = (a, b) if a.n <= b.n else (b, a)
                return V(a.n * b.n, '(f 23 %s %s)' % (lo.t, hi.t))
            pin(a), pin(b)                  # heavy: pin the factor expressions, witness the product
            prod = wit_mul(a.n, b.n)
            return V(prod, unat(prod))
        if h == '-':
            a, b = num(ev(e[1], env)), num(ev(e[2], env))
            if zpair:                       # (p1-n1) - (p2-n2) = (p1+n2) - (n1+p2)
                return V(a.n - b.n, '(f 21 %s %s)' % (a.t, b.nt), '(f 21 %s %s)' % (a.nt, b.t))
            if a.n < b.n:
                if binary:
                    raise Out('underflow in binary mode')
                raise Under()               # retry the whole sample with ℤ difference-pair values
            if binary:
                d = a.n - b.n
                pin(a), pin(b)              # subtraction verified BY ADDITION: d + b = a
                vcs.append('(= (f 81 %s %s) %s) (refl %s)'
                           % (bin_lit(d), bin_lit(b.n), bin_lit(a.n), bin_lit(a.n)))
                return V(d, bin_lit(d))
            if b.n <= 200 and a.n <= 3000:
                # DOMAIN-ERASURE WITNESS: omega2gamma drops `in Saturating`/`Wrapping` annotations, which
                # is sound only where the domains AGREE with plain arithmetic — for subtraction, exactly
                # when no underflow occurred. The kernel re-checks ult(a, b) = 0 (b <= a) at every erased
                # site; the witnessed heavy path proves the same via its d + b = a certificate.
                pin(a), pin(b)
                vcs.append('(= (f 28 %s %s) (k 2)) (refl (k 2))' % (unat(a.n), unat(b.n)))
                return V(a.n - b.n, '(f 22 %s %s)' % (b.t, a.t))   # usub(b, a) = a - b, in-envelope
            pin(a), pin(b)                  # heavy: subtraction VERIFIED BY ADDITION — d + b = a
            d = a.n - b.n
            lo, hi = (d, b.n) if d <= b.n else (b.n, d)
            wit_sum(chunks(lo), hi)
            return V(d, unat(d))
        if h in ('/', '%'):
            # DIVISION BY WITNESS (certifying computation): the fueled udiv/umod reduction blows the
            # envelope — the old quotient wall. Instead the encoder WITNESSES q and r and the kernel checks
            # the DEFINING PROPERTY across in-envelope certs:
            #   iszero(divisor) = 0            (safety: the division cannot trap)
            #   pins + chunked literal certs   (q * divisor + r = dividend, the Euclidean decomposition)
            #   ult(r, divisor) = 1            (r is the true remainder => q, r are UNIQUE)
            # Uniqueness of Euclidean division makes the literals q/r kernel-VERIFIED, not trusted; the
            # result term is the literal, so downstream arithmetic composes from a checked value.
            a, b = num(ev(e[1], env)), num(ev(e[2], env))
            if zpair:
                raise Out('division over difference pairs: later')
            if b.n == 0:
                raise Out('division by zero')
            q, r = a.n // b.n, a.n % b.n
            if binary:
                pin(a), pin(b)
                vcs.append('(= (f 81 %s %s) %s) (refl %s)'   # r + (b-r) = b: r < b and b nonzero
                           % (bin_lit(r), bin_lit(b.n - r), bin_lit(b.n), bin_lit(b.n)))
                if q:
                    lo, hi = (q, b.n) if q <= b.n else (b.n, q)
                    vcs.append('(= (f 81 (f 87 %s %s) %s) %s) (refl %s)'
                               % (bin_lit(lo), bin_lit(hi), bin_lit(r), bin_lit(a.n), bin_lit(a.n)))
                return V(q if h == '/' else r, bin_lit(q if h == '/' else r))
            if r > CHUNK:
                raise Out('remainder %d exceeds the chunk envelope' % r)
            vcs.append('(= (f 24 %s) (k 2)) (refl (k 2))' % b.t)
            pin(a), pin(b)
            prod = wit_mul(q, b.n)          # q * b, literal certs
            if q:
                add_cert(r, prod)           # q * b + r = a (values pinned to the terms above)
            vcs.append('(= (f 28 %s %s) (k 3 (k 2))) (refl (k 3 (k 2)))' % (unat(r), unat(b.n)))
            return V(q if h == '/' else r, unat(q if h == '/' else r))
        if h in ('<', '<=', '==', '!=', 'eq', 'lt', 'le', 'ne'):    # comparisons decided concretely
            a, b = num(ev(e[1], env)), num(ev(e[2], env))
            r = {'<': a.n < b.n, '<=': a.n <= b.n, '==': a.n == b.n, '!=': a.n != b.n,
                 'eq': a.n == b.n, 'lt': a.n < b.n, 'le': a.n <= b.n, 'ne': a.n != b.n}[h]
            return V(1 if r else 0, lit(1 if r else 0), Z0 if zpair else None)
        if h == 'let':
            env2 = dict(env)
            env2[e[1]] = ev(e[2], env)
            return ev(e[3], env2)
        if h == 'if':
            c = num(ev(e[1], env))
            return ev(e[2] if c.n != 0 else e[3], env)
        if h == 'match':
            sub = ev_ctor(e[1], env)
            for arm in e[2:]:
                pat, body = arm[0], arm[1]
                bound = match(pat, sub)
                if bound is not None:
                    env2 = dict(env)
                    env2.update(bound)
                    return ev(body, env2)
            raise Out('no match arm fired')
        if isinstance(h, str) and h in defs:
            params, body = defs[h]
            args = [ev_ctor(x, env) for x in e[1:]]
            if len(args) != len(params):
                raise Out('arity mismatch calling %s' % h)
            if h in ('nth', 'setl'):
                if not inarr[0]:
                    arr_vc(args[0], args[1])
                inarr[0] += 1
                try:
                    return ev(body, dict(zip(params, args)))
                finally:
                    inarr[0] -= 1
            return ev(body, dict(zip(params, args)))
        if isinstance(h, str) and h[0].isupper():
            return ev_ctor(e, env)
        raise Out('form %s outside the fragment' % h)

    def ev_ctor(e, env):                   # values may be constructor applications (Pair etc.)
        if isinstance(e, list) and e and isinstance(e[0], str) and e[0][0].isupper():
            return (e[0],) + tuple(ev_ctor(x, env) for x in e[1:])
        return ev(e, env)

    def match(pat, val):
        if isinstance(pat, str):
            if pat[0].isupper():
                return {} if val == pat or (isinstance(val, tuple) and val[0] == pat and len(val) == 1) else None
            return {pat: val}
        if isinstance(pat, list):
            if not (isinstance(val, tuple) and val[0] == pat[0] and len(val) == len(pat)):
                return None
            bound = {}
            for p2, v2 in zip(pat[1:], val[1:]):
                b2 = match(p2, v2)
                if b2 is None:
                    return None
                bound.update(b2)
            return bound
        return None

    r = ev(top, {})
    if not isinstance(r, V):               # a constructor tree: emit the STRUCTURAL claim
        # ℤ-pair mode: leaves carry (pos, neg) difference-pair terms that can't sit inside one tree
        # equality. Each leaf gets its own PIN — (= t (f 21 unat(n) nt)), the kernel verifying
        # pos - neg = n in ℤ with the rhs term as its own refl witness — and the tree claim rides the
        # literals; the render pin covers the shape against the interpreter's printed value.
        def zleaf(v):
            litv = unat(v.n)
            if v.nt is not None and v.t != litv:
                rhs_ = '(f 21 %s %s)' % (litv, v.nt)
                vcs.append('(= %s %s) (refl %s)' % (v.t, rhs_, rhs_))
            return litv

        def stt(v, concrete):              # the tree as a kernel term; leaves computed (lhs) or literal (rhs)
            if isinstance(v, V):
                if zpair:
                    return zleaf(v) if not concrete else unat(v.n)
                return lit(v.n) if concrete else v.t
            if isinstance(v, str):
                return '(k %d)' % ctor_cid(v, 0)
            cid = ctor_cid(v[0], len(v) - 1)
            return '(k %d %s)' % (cid, ' '.join(stt(x, concrete) for x in v[1:]))

        def render(v):                     # interp.beta's exact print format, for the gate's stdout pin
            if isinstance(v, V):
                return str(v.n)
            if isinstance(v, str):
                return v
            return '(%s)' % ' '.join([v[0]] + [render(x) for x in v[1:]])

        lhs, rhs = stt(r, False), stt(r, True)
        pre = (BIN_PRELUDE + ' ') if binary else (USER_PRELUDE + ' ') if user else ''
        decls = ' '.join('(data %d %d 0 0)' % (cid, ar) for cid, ar in sorted(ctors.values()))
        badcid = 100 + len(ctors)           # a fresh constructor nothing equals: the negative control
        # dual-channel results: (Pair <exit> <stdout>) reports the exit component as the exit code (the
        # interpreter exits 0 for any constructor value; the gate parses the same pair from its stdout)
        sexit = r[1].n & 0xFF if (r[0] == 'Pair' and len(r) == 3 and isinstance(r[1], V)) else 0
        if r[0] == 'Pair' and len(r) == 3 and isinstance(r[1], V):
            boundary_vc(r[1].n)            # the exit component crosses the boundary...
            seen_bytes = set()

            def bwalk(v):                  # ...and so does every stdout byte in the output list
                if isinstance(v, V):
                    if v.n not in seen_bytes:
                        seen_bytes.add(v.n)
                        boundary_vc(v.n)
                elif isinstance(v, tuple):
                    for x in v[1:]:
                        bwalk(x)
            bwalk(r[2])
        print('%d %s%s (= %s %s) (refl %s)' % (sexit, pre, decls, lhs, rhs, rhs))
        print('%s%s (data %d 0 0 0) (= %s (k %d)) (refl (k %d))' % (pre, decls, badcid, lhs, badcid, badcid))
        for vc in dict.fromkeys(vcs):
            print('%s%s' % (pre, vc))
        print('#render %s' % render(r))    # pins the structure to the interpreter's printed value
        return
    if zpair:                              # claim P = uadd(exit, N): verifies pos - neg = exit in ℤ
        if not (0 <= r.n <= 255):
            raise Out('final ℤ value %d not a plain exit byte' % r.n)
        boundary_vc(r.n)
        rhs = '(f 21 %s %s)' % (unat(r.n), r.nt)
        bad = '(f 21 %s %s)' % (unat(r.n + 1), r.nt)
        print('%d %s (= %s %s) (refl %s)' % (r.n, USER_PRELUDE, r.t, rhs, rhs))
        print('%s (= %s %s) (refl %s)' % (USER_PRELUDE, r.t, bad, bad))
        for vc in dict.fromkeys(vcs):      # lines 3+: safety obligations (array bounds in zpair mode)
            print('%s %s' % (USER_PRELUDE, vc))
        return
    boundary_vc(r.n)                       # the exit code crosses the boundary: it must be a byte
    exit_code = r.n & 0xFF
    good = lit(r.n)
    bad = lit(r.n + 1)                     # the off-by-one negative control, in the mode's encoding
    pre = (BIN_PRELUDE + ' ') if binary else (USER_PRELUDE + ' ') if user else ''
    print('%d %s(= %s %s) (refl %s)' % (exit_code, pre, r.t, good, good))
    print('%s(= %s %s) (refl %s)' % (pre, r.t, bad, bad))
    for vc in dict.fromkeys(vcs):          # lines 3+: the division-safety obligations, each kernel-checked
        print('%s%s' % (pre, vc))


if __name__ == '__main__':
    try:
        main()
    except Out as e:
        sys.stderr.write('outside fragment: %s\n' % e)
        sys.exit(2)
    except RecursionError:
        sys.stderr.write('outside fragment: recursion depth\n')
        sys.exit(2)
