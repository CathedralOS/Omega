# Instruction-Level Refinement — proving bc's machine code computes its source meaning

> **The deepest realization of the lattice's end goal** — *the output certifies the compiler.* The meaning
> route (gamma) certifies what a program's SOURCE means; translation validation certifies each compilation's
> RESULT; this reaches the bottom rung — the actual **machine code** the alpha VM executes — and certifies it
> computes exactly the function its Beta source denotes, **for all inputs, without ever running it.** Gate:
> [`refinement.sh`](refinement.sh), wired into `verify-lattice.sh`.

## What it proves

For a Beta program `P`, let `bc(P)` be the alpha bytecode the compiler emits. The gate establishes, fully
automatically and with no human-written specification:

```
    ∀ inputs.   ⟦ bc(P) ⟧_alpha   =   ⟦ P ⟧_Beta
```

— the compiled machine code and the source program denote the *same function of the inputs* — and it does so
by a **kernel-checked proof**, not by testing. A compiler that miscompiled `P` would produce a `bc(P)` whose
meaning differs, and the proof would fail.

## Architecture — two independent symbolic evaluators, kernel-checked equal

The certificate is a diamond with the trust anchor (`delta/check.beta`) at the point:

```
      Beta source P ──beta_symbolic──▶  M = ⟦P⟧_Beta  (a closed-form term over the inputs)
            │                                   │
            │ bc + assembler                    │  prove  (= C M)  ∀ inputs
            ▼                                   │  via delta/prover.py → check.beta
      alpha bytecode ──alpha_symbolic──▶ C = ⟦bc(P)⟧_alpha
            │                                   │
            └── pinned to alpha_ref.py          └── pinned to beta_interp.py
                (the real VM) on random inputs      (the real interpreter) on random inputs
```

Both symbolic evaluators are **UNTRUSTED and checked**, exactly like the other `*_ref` / `*_symbolic` tools
(`alpha_ref.py`, `asm_ref.py`, `bc2.py`, `gamma_ref.py`, `check_ref.py`). Two independent checks make a bug in
either evaluator — or a real miscompile — surface loudly:

1. **Differential pinning.** `C` evaluated at random inputs must equal what the actual bytecode does on the
   alpha VM (`alpha_ref.py`); `M` must equal what the source does in the interpreter (`beta_interp.py`). Each
   derivation is tied to its *own* reference, so neither can drift from ground truth undetected.
2. **Kernel-checked equivalence.** `∀ inputs. C = M` is proved and the proof validated by the trust anchor.
   The proof ties `C` (what the machine does) to `M` (what the source means); the two pins ground both ends.
   A perturbed meaning `(s M)` is required to be *unprovable* (the teeth).

Because both ends are independently derived and the equivalence is kernel-proved, a passing check certifies the
**compiler**, not a tautology.

## The meaning language

A meaning is a term over: `z`/`(s t)` (Peano nat), `(p a b)` plus, `(m a b)` times, `(v i)` the i-th input,
and `(f ID t)` a **user-function recurrence** — currently only `TRI_ID = 90`, the triangular sum
`g(0)=0, g(s k)=g(k)+k` so `g(n)=Σ_{j<n} j`. The recurrence's `(fun ..)` definition is prepended to the
certificate (`REC_PRELUDE`); the checker accepts a recurrence applied to a *symbolic* input by refl.

## The covered fragment

`bc(P)` is certified when `P` is in the fragment both evaluators model exactly:

| Feature | Handled how |
| --- | --- |
| Straight-line `+`, `*` over inputs / constants | direct symbolic execution (exact) |
| `read_byte()` | a fresh input variable `(v i)` |
| Function calls, recursion (concrete depth) | inlined / unrolled during symbolic execution |
| Concrete-bounded loops (`state` machines) | unrolled — control flow is data-independent |
| **Data-dependent counter loops** `i < n` / `i <= n` | **summarized** to a closed form without unrolling |
| — accumulator `acc += a0 + a1·i` (linear in the counter) | `init + a0·trip + a1·g(trip)` — covers invariant deltas, `acc += i` (Σi), `a·i`, `a+i`, weighted/offset series |
| Composition (pre-loop arith → loop → post-loop arith) | the summarized loop result flows through further terms |
| Straight-line subtraction `a - b` (may underflow) | a **ℤ difference-pair** `(k 5 pos neg) = pos - neg` (see below) |
| Subtracting loop accumulators `acc = acc - δ` (δ linear in `i`) | the pair's pos/neg components follow **independent additive recurrences** — each summarizes as its own series |
| **Down-counting loops** `i = n; while (0 < i): …; i -= 1` | exactly `n` trips (the counter drains by the ℤ pair −1); deltas must be loop-invariant — the counter's value is `n−k`, not `k` |

Linear-in-counter deltas (`a·i`, `a+i`, `(a·i)+b`, …) refine **end to end**. Both engines decompose the
per-iteration delta into `a0 + a1·i` (via `_lin_decompose` + `_series_closed`): `beta_symbolic` reads the delta
symbolically off one placeholder body run; `alpha_symbolic` uses three-iteration finite differences for the
invariant / pure-Σi fast path, falling back to a **placeholder body iteration** (every frame slot set to a
`('slot', addr)` marker, invariant slots substituted back, the rest decomposed over the counter's marker) for
the general case. The shared `_canon` normalizer keeps both engines' coefficient forms byte-identical.

### Straight-line subtraction via ℤ difference-pairs

Peano has no negatives and alpha's `sub` opcode wraps mod 2⁶⁴, so `a - b` can't be a bare Peano term. Both
engines instead carry a value as a **difference pair** `('zz', pos, neg)` meaning `pos - neg`, rendered
`(k 5 pos neg)` and cert-checked by `refl` (prelude `(data 5 2 0 0)`). Arithmetic distributes over the pair
(`(pa-na)+(pb-nb)`, `(pa-na)(pb-nb)`, `(pa-na)-(pb-nb)`); a value only becomes `zz` once a subtraction touches
it, so pure `+`/`*`/loop paths are byte-for-byte unchanged. **Soundness:** the observable is mod 256 and
256 | 2⁶⁴, so ℤ arithmetic reduced mod 256 equals alpha's mod-2⁶⁴ arithmetic reduced mod 256 — the pair and
the wrapped machine value agree on every observed byte, including underflow (`0-1 → (k 5 z (s z))`, observed
`255`). `_sub` is **identical in both engines**: a non-underflowing concrete literal difference folds to a
nat (matching how `+`/`*` fold), an underflowing one becomes a small pair, and any symbolic operand takes the
pair path. The concrete-fold branch also serves alpha's frame addressing (base − offset via the `sub`
opcode), which never underflows.

Inside a loop, a subtracting accumulator works because `+`/`-` distribute **componentwise** over difference
pairs: after one placeholder body run the new value is `('zz', ph + Dp, N)`, so the pos component gains `Dp`
and the neg component gains `N` each iteration — two independent additive recurrences, each summarized by the
existing linear machinery (`drain`: `total = n·a` then `-= a` over `i<n` → `(k 5 (m n a) (p z (m n a)))`;
`total -= i` → neg component `g(n)`). Both engines also refuse a zz value *nested inside* a delta component
(forms would diverge), and a decreasing **concrete** slot no longer wraps to a 2⁶⁴-scale coefficient (whose
Peano rendering is unbuildable) — it is routed to the placeholder path instead.

Down-counting needs no new guard operator: `while (0 < i)` puts the concrete 0 on the compare's left, which
both recognizers already accept; the counter is the slot stepping by the pair −1 whose entry value is the
guard's right side, and the trip count is that entry value. `0 <= i` with a −1 step never terminates and is
not recognized; counter-dependent deltas under a down-counter are refused (the counter's value at iteration
`k` is `n−k`, so the up-count series would be wrong — a later slice can substitute `i ↦ n−k`).

**Deliberately out of scope** (each conservatively *refused* — never mis-summarized): scaling recurrences
(`acc = (acc-1)·2`); division; *genuinely* non-linear counter deltas (`i·i`, `i·total`); counter-dependent
deltas under a down-counter; ℤ-pair trip counts; byte-granular memory; nested / multi-loop recurrences.

## How data-dependent loops are summarized (the interesting part)

A symbolic trip count `n` can't be unrolled. Both sides recognize the loop and replace it with a closed form.

- **Source (`beta_symbolic`)** — the AST makes the loop explicit. It runs the body ONCE with each loop
  variable set to a fresh placeholder, reads off each per-iteration *delta*, and (for a unit-stride counter
  from 0 with a loop-invariant delta) emits `init + trip·delta`; for `delta == the counter`, `init + g(trip)`.
- **Bytecode (`alpha_symbolic`)** — the loop-carried variables live in frame memory slots and the guard is a
  multi-instruction `load`/`jlt`/`jz` sequence. It pre-scans for the back-edge (a `jmp` to a lower address),
  recognizes bc's `<`/`<=` compare-to-boolean idiom, and at the guarding `jz` runs the body **three** times on
  copies, taking **finite differences** of each frame slot: increments `(d,d,d)` ⇒ an invariant delta,
  `(0,1,2)` ⇒ `delta(k)=k` (Σi). One body iteration is straight-line, so each increment is derived *exactly*,
  not sampled. A mis-recognition is caught by the differential-across-trip-counts pin; a failed recognition
  bails — never a false certificate.

## File map

| File | Role |
| --- | --- |
| `alpha_symbolic.py` | UNTRUSTED: symbolically executes an alpha tape → the compiled meaning `C`. Dual concrete-int / Peano-term values; concrete-addressed memory + call/ret; loop summarization. |
| `../beta-lang-py/beta_symbolic.py` | UNTRUSTED: symbolically evaluates a Beta source → the source meaning `M`. The source-side dual; reuses `bc2`'s parser. |
| `alpha_refinement_check.py` | The gate driver: derives `C`/`M`, differentially pins each, proves `(= C M)`. Curated samples + three fuzz spaces. |
| `refinement_fuzz_gen.py` | random straight-line arithmetic programs |
| `refinement_loop_gen.py` | random data-dependent counter loops (`<` / `<=`) |
| `refinement_compose_gen.py` | random pre-loop + loop + post-loop compositions |
| `refinement.sh` | builds `check.beta` + `bc`, runs the driver; the lattice step |
| `refinement-samples/*.beta` | curated end-to-end samples (muln, countn, tri, muln_le, …) |
| `../beta-lang-py/symbolic_loop_check.py` + `symbolic-loops.sh` | source-side soundness gate: `beta_symbolic`'s loop summaries pinned to `beta_interp` over an input grid |

## Reference producers (never run in the trusted lineage)

`alpha_ref.py` (the alpha VM in Python) and `beta_interp.py` (the Beta interpreter) are the ground-truth
references the two symbolic evaluators are pinned against. `delta/prover.py` searches for the equality proof;
`delta/check.beta` (the trust anchor) validates it.
