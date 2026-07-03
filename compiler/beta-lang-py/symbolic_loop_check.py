#!/usr/bin/env python3
# symbolic_loop_check.py — soundness gate for beta_symbolic's DATA-DEPENDENT loop summarization. beta_symbolic
# derives a closed-form, all-inputs meaning for a linear counter loop (`total += c` while `i < n`) WITHOUT
# unrolling — a symbolic trip count. This gate pins that closed form to the ground truth: the concrete Beta
# interpreter (beta_interp.py) run at every point of an input grid. If the summarizer's closed form ever
# disagrees with actually running the loop, it is caught here. It also checks that loops OUTSIDE the recognized
# linear pattern (e.g. `total += i`, a triangular sum) are conservatively REFUSED, not silently mis-summarized.
#
# This is the source-side half of data-dependent-loop refinement; the bytecode half (alpha_symbolic) is a
# later slice. UNTRUSTED and checked, like the other *_symbolic / *_ref tools.
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import beta_symbolic as B
import beta_interp
from bc2 import lex, Parser

def loop(guard, updates, ret='total'):
    return ("proc main() {\n"
            "    let n = read_byte()\n    let a = read_byte()\n    let b = read_byte()\n"
            "    let total = 0\n    let s = 0\n    let i = 0\n"
            "    state loop { to body when (%s)  return %s }\n"
            "    state body { %s  i = i + 1  to loop }\n}\n") % (guard, ret, updates)

def downloop(updates, ret='total'):    # i drains n -> 0 under (0 < i): exactly n trips
    return ("proc main() {\n"
            "    let n = read_byte()\n    let a = read_byte()\n    let b = read_byte()\n"
            "    let total = 0\n    let s = 0\n    let i = n\n"
            "    state loop { to body when (0 < i)  return %s }\n"
            "    state body { %s  i = i - 1  to loop }\n}\n") % (ret, updates)

def nestedloop(inner_bound):           # outer i < n (symbolic trip); inner j < inner_bound; total += a inside
    return ("proc main() {\n"
            "    let n = read_byte()\n    let a = read_byte()\n    let b = read_byte()\n"
            "    let total = 0\n    let i = 0\n"
            "    state outer { to obody when (i < n)  return total }\n"
            "    state obody { let j = 0  to inner }\n"
            "    state inner { to ibody when (j < %s)  to onext }\n"
            "    state ibody { total = total + a  j = j + 1  to inner }\n"
            "    state onext { i = i + 1  to outer }\n}\n") % inner_bound

# (name, source, expected closed-form value fn(n,a,b))  — summarizable linear loops
SUMMARIZABLE = [
    ("n*a   (i<n, total+=a)",   loop("i < n",  "total = total + a"),              lambda n, a, b: n * a),
    ("n     (i<n, total+=1)",   loop("i < n",  "total = total + 1"),              lambda n, a, b: n),
    ("(n+1)*a (i<=n, total+=a)", loop("i <= n", "total = total + a"),             lambda n, a, b: (n + 1) * a),
    ("n*(a+b) (i<n, +=a +=b)",  loop("i < n",  "total = total + a  s = s + b", 'total'),
     lambda n, a, b: n * a),                                                       # returns total = n*a (s unused in ret)
    ("n*a + init (total0=... )", loop("i < n", "total = total + a"),              lambda n, a, b: n * a),
    ("Σi  (i<n, total+=i)",     loop("i < n",  "total = total + i"),              lambda n, a, b: n * (n - 1) // 2),
    ("Σi  (i<=n, total+=i)",    loop("i <= n", "total = total + i"),              lambda n, a, b: n * (n + 1) // 2),
    ("Σi + n*a (two accum)",    loop("i < n",  "total = total + i  s = s + a", 'total'),
     lambda n, a, b: n * (n - 1) // 2),
    ("a·Σi  (total += a*i)",    loop("i < n",  "total = total + (a * i)"),         lambda n, a, b: a * (n * (n - 1) // 2)),
    ("n·a + Σi (total += a+i)", loop("i < n",  "total = total + (a + i)"),         lambda n, a, b: n * a + n * (n - 1) // 2),
    ("2i+3 (total += (2*i)+3)", loop("i < n",  "total = total + ((2 * i) + 3)"),   lambda n, a, b: 2 * (n * (n - 1) // 2) + 3 * n),
    # SUBTRACTING accumulators: the value is a ℤ difference pair; its pos/neg components summarize as
    # independent series, and the observable (mod 256) matches the interpreter's wrapped byte exactly.
    ("-n*a  (total -= a, ℤ)",   loop("i < n",  "total = total - a"),               lambda n, a, b: (-n * a) % 256),
    ("-Σi   (total -= i, ℤ)",   loop("i < n",  "total = total - i"),               lambda n, a, b: (-(n * (n - 1) // 2)) % 256),
    ("n(a-1) (total += a then -1)", loop("i < n", "total = ((total + a) - 1)"),    lambda n, a, b: (n * (a - 1)) % 256),
    # DOWN-counting loops: guard (0 < i), i starts at n and steps by the ℤ pair -1 — exactly n trips.
    ("↓ n*a  (0<i, total+=a)",  downloop("total = total + a"),                     lambda n, a, b: n * a),
    ("↓ -n*a (0<i, total-=a)",  downloop("total = total - a"),                     lambda n, a, b: (-n * a) % 256),
    ("↓ ret i (drained counter)", downloop("total = total + a", ret='i'),          lambda n, a, b: 0),
    # counter-DEPENDENT deltas under a down-counter: i ↦ n-k folds the linear part into the invariant
    # coefficient and flips the triangular part's sign across the pair — Σ of i for i=n..1 is n² - g(n).
    # swapped-operand guard spellings: (a > b) ≡ (b < a), normalized before recognition (bc's codegen does
    # the same swap, so the bytecode side never sees > at all).
    ("↓ (i > 0) n*a",           downloop("total = total + a").replace('0 < i', 'i > 0'),
     lambda n, a, b: n * a),
    ("(n > i) n*a  (≡ i<n)",    loop("n > i",  "total = total + a"),               lambda n, a, b: n * a),
    ("(n >= i) (n+1)*a (≡ i<=n)", loop("n >= i", "total = total + a"),             lambda n, a, b: (n + 1) * a),
    # != guards: over ℕ with a unit-stride counter, != IS < (exact hit, no overshoot) — normalized likewise.
    ("(i != n) n*a  (≡ i<n)",   loop("i != n", "total = total + a"),               lambda n, a, b: n * a),
    ("(n != i) n*a  (≡ i<n)",   loop("n != i", "total = total + a"),               lambda n, a, b: n * a),
    ("↓ (i != 0) n*a (≡ 0<i)",  downloop("total = total + a").replace('0 < i', 'i != 0'),
     lambda n, a, b: n * a),
    # NESTED, inner bound concrete: the outer body's placeholder run UNROLLS the inner loop, leaving an
    # additive spine ((total+a)+a)+a whose delta peels to (a+a)+a — closed form n·3a without new theory.
    ("nested ×3 (inner concrete)", nestedloop("3"),                                lambda n, a, b: 3 * n * a),
    # NESTED, inner bound SYMBOLIC: the inner loop is summarized RECURSIVELY during the outer body's
    # placeholder run — its closed form (over the outer run's markers) becomes the outer delta. The
    # triangular case (inner bound = the outer counter) yields the counter-linear delta a·i -> a·g(n).
    ("nested n·b·a (inner j<b)",  nestedloop("b"),                                 lambda n, a, b: n * b * a),
    ("nested triangular (j<i)",   nestedloop("i"),                                 lambda n, a, b: a * (n * (n - 1) // 2)),
    ("nested n·g(b) (total+=j)",  nestedloop("b").replace('total = total + a', 'total = total + j'),
     lambda n, a, b: n * (b * (b - 1) // 2)),
    ("↓ Σi   (total += i)",     downloop("total = total + i"),                     lambda n, a, b: n * (n + 1) // 2),
    ("↓ -Σi  (total -= i)",     downloop("total = total - i"),                     lambda n, a, b: (-(n * (n + 1) // 2)) % 256),
    ("↓ a·Σi (total += a*i)",   downloop("total = total + (a * i)"),               lambda n, a, b: a * (n * (n + 1) // 2)),
]
# loops the recognizer must REFUSE (genuinely non-linear in the counter) -> beta_symbolic raises Unsupported
MUST_REFUSE = [
    ("total += (i*i) (counter²)",     loop("i < n", "total = total + (i * i)")),
    ("total += (a*total) (nonlinear)", loop("i < n", "total = total + (a * total)")),
    ("total = (total-1)*2 (scaling)", loop("i < n", "total = ((total - 1) * 2)")),
    ("↓ total += (i*i) (counter²)",   downloop("total = total + (i * i)")),
    # != with a stride that can SKIP the bound: i jumps over n when n is odd and the machine diverges — the
    # unit-stride requirement refuses it, which is exactly what makes the != ≡ < normalization sound.
    ("(i != n) stride 2 (skips!)",    loop("i != n", "total = total + a").replace('i = i + 1', 'i = i + 2')),
    # Σ of g: inner (j < i, total += j) makes the outer delta g(i) — quadratic in the outer counter
    # (tetrahedral sum), genuinely outside the linear class on both sides.
    ("nested Σg (j<i, total+=j)",     nestedloop("i").replace('total = total + a', 'total = total + j')),
]

def main():
    fails = 0; n_checks = 0
    for name, src, fn in SUMMARIZABLE:
        try:
            M, ninp = B.meaning(src)
        except B.Unsupported as e:
            print("  FAIL %-28s : refused a summarizable loop (%s)" % (name, e)); fails += 1; continue
        procs = Parser(lex(src)).parse()
        bad = 0
        for nn in range(0, 14):
            for aa in range(0, 10):
                for bb in (0, 3):
                    if nn * (aa + bb) >= 256:
                        continue
                    v = B.evaluate(M, {0: nn, 1: aa, 2: bb}) % 256
                    rc, _ = beta_interp.interpret(procs, bytes([nn, aa, bb]))
                    n_checks += 1
                    if v != rc:
                        bad += 1
                        if bad <= 2:
                            print("  MISMATCH %-24s n=%d a=%d b=%d sym=%d interp=%d" % (name, nn, aa, bb, v, rc))
        if bad:
            fails += 1
        else:
            print("  ok   %-28s : closed form %s pinned to interpreter" % (name, B.render(M)))
    for name, src in MUST_REFUSE:
        try:
            B.meaning(src)
            print("  FAIL %-28s : summarized a loop outside the linear class (unsound!)" % name); fails += 1
        except B.Unsupported:
            print("  ok   %-28s : correctly refused (conservative)" % name)
    print("symbolic loop summarization: %d input-grid checks, %d failures" % (n_checks, fails))
    sys.exit(1 if fails else 0)

if __name__ == '__main__':
    main()
