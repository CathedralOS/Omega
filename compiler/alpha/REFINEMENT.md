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

The certificate is a diamond with the trust anchor (`proof-kernel/check.beta`) at the point:

```
      Beta source P ──beta_symbolic──▶  M = ⟦P⟧_Beta  (a closed-form term over the inputs)
            │                                   │
            │ bc + assembler                    │  prove  (= C M)  ∀ inputs
            ▼                                   │  via proof-kernel/prover.py → check.beta
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
| **Down-counting loops** `i = n; while (0 < i): …; i -= 1` | exactly `n` trips (the counter drains by the ℤ pair −1) |
| — counter-dependent deltas under a down-counter | `i ↦ n−k`: the linear part folds into the invariant coefficient, the triangular part flips sign across the pair (`total += i` ↓ → `n² − g(n)` = `n(n+1)/2`) |

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
guard's right side, and the trip count is that entry value. The `>`/`>=` spellings also work: `(a > b)` ≡
`(b < a)` — bc's codegen swaps the operands into the same `jlt` idiom (so the bytecode recognizer never sees
`>` at all) and `beta_symbolic` normalizes the source guard the same way before recognition.

`!=` guards normalize too, because over ℕ with a **unit-stride** counter `!=` *is* `<`: `i != n` from 0 by
+1 hits `n` exactly (never skips), and `i != 0` by −1 drains to 0 exactly. The counter checks the normalized
branches already enforce (entry 0, stride ±1) are precisely what makes the exact-hit argument sound — a
stride that could skip the bound (`i += 2`, where the machine diverges for odd `n`) fails them and refuses.
The up-count path also requires the counter to **enter at 0** on both sides now (`trip = bound` is only
correct from 0; alpha previously accepted any concrete entry, a wrong-summary latent bug the differential
pin would have caught downstream). `0 <= i` with a −1 step never terminates and is
not recognized. Counter-dependent deltas work through the substitution `i ↦ n−k`: a component `a0 + a1·i`
sums to `(a0 + a1·n)·t − a1·g(t)`, so `_down_series` folds the linear part into the invariant coefficient
and routes each component's `g` cross-term to the *other* side of the pair (shared recipe in both engines).

### Nested loops

`while (i < n) { 3× total += a }` certifies without new closed-form theory: the outer body's single
placeholder run **unrolls the inner loop concretely** (alpha's `_run_body_once` gained concrete-only
conditional branches; beta's summarizer walks the multi-state body *region* with the same semantics as its
main interpreter), leaving the additive spine `((total + a) + a) + a`. The shared `_peel` extracts the
per-iteration delta `(a + a) + a` from any such spine — left-first, preserving tree shape so both engines
build identical terms — and the ordinary series machinery does the rest: `n · ((a+a)+a)`. Body-introduced
vars (the inner counter `j`) are kept after the loop only if their value is iteration-independent; the rest
are dropped, and a post-loop read of a dropped var refuses.

A **symbolic** inner bound summarizes **recursively**: when the body run meets an inner symbolic guard, the
same summarizer runs at depth+1 (both engines; depth capped at 8), and the inner closed form — expressed
over the *outer* run's markers — becomes the outer per-iteration delta. Markers of slots the inner loop
doesn't move pass through its decompose as opaque constants (`_lin_decompose` takes the *moved* set, so an
outer marker is "invariant here") and resolve at the outer level. This composes with everything the single-
loop machinery knows: `j < m` gives the invariant delta `m·a` → `n·(m·a)`; `total += j` inside gives
`g(m)` → `n·g(m)`; and the **triangular** `j < i` (inner bound = outer counter) makes the outer delta
counter-linear `a·i` → `a·g(n)`. `Σ g(i)` (inner `j < i, total += j`) is quadratic in the outer counter —
refused on both sides. `refinement_nested_gen.py` fuzzes this recursive machinery: random nested shapes over
all four inner-bound kinds (concrete, input, the outer counter) with subtracting bodies and outer-step
deltas, each proven against the compiled bytecode.

### Calls in loop bodies and rewrite slots

A procedure call inside a summarized body is **inlined during the placeholder run** (alpha's body runner
gained call/ret; beta's `ev` always inlined), so `total += double(i)` certifies as the counter-linear
`2·g(n)`. The callee's temp slots — and any per-iteration temporary like `t = a·i` — are **rewrite slots**:
fully overwritten each iteration, no additive delta exists for them. Instead of refusing the loop, both
engines DROP them post-loop (alpha poisons the slot — a later load refuses; beta removes the var — a later
read refuses) and refuse only if another delta reads a rewrite slot's *stale* previous-iteration value.

The gate's differential pin runs the compiled tape on the reference VM with a **timeout**: a diverging tape
(found in the wild — bc emits divergent code for an assignment to an undeclared variable) fails the pin
loudly instead of hanging the gate.

### Byte memory (straight-line, concrete addresses)

`byte[5000] = a + b; return byte[5000] * 2` certifies: both engines model a byte map at concrete addresses.
A stored **symbolic** value is kept *untruncated* even though `storeb` keeps only the low byte — sound
because the observable is mod 256 and `+`/`−`/`*` respect mod-256 congruence (a ring homomorphism ℤ→ℤ/256),
so every observed byte matches the machine; concrete stores truncate exactly. Alpha's initial byte memory is
the **tape image** (as on the machine) while the source interpreter's is zeroed — a low-address program
would honestly FAIL the cross-engine proof, which is correct: it genuinely behaves differently interpreted
vs compiled. Word↔byte aliasing refuses on both directions; `word[..]` and byte ops inside loops are later
slices.

### Monus trip counts (symbolic counter starts)

`i = a; while (i < n)` runs **n ∸ a** times — truncated subtraction, the constructor `(k 6 n a)` — which is
the *branch-free* trip count: `a > n` gives 0 iterations on the machine and `0 = n ∸ a` in ℕ alike, so the
closed form holds for **all** inputs with no case split. Counter-linear deltas fold the start offset into
the invariant coefficient (`Σᵢ₌ₐⁿ⁻¹ i = (n∸a)·a + g(n∸a)`; identity fold when the start is 0, so all prior
forms are byte-stable). Like the ℤ pair, monus is a plain binary constructor to the kernel (`(data 6 2 0 0)`,
certs by refl); its meaning lives in the two differentially-pinned evaluators. `!=` guards refuse from a
nonzero start — the machine genuinely diverges there when `start > bound`.

### The input stream (read-loops)

`while (i < n): total += read_byte()` certifies as `Σ input[1..1+n)` — the stream-sum constructor
`(k 8 lo hi)`, with individual symbolic-position reads as stream elements `(k 7 t)`. The mechanism: the read
**position** is a hidden loop variable (a virtual frame slot `RDV` on the machine side), so the ordinary
summarizer handles it — delta +1 per iteration, markers, series closure `base + trip` — with no
special-purpose recognizer. Fixed-index reads stay `(v k)`, keeping all prior forms unchanged. Post-loop
reads work (`(k 7 (p 1 n))` = the next byte after the loop); discarded reads just advance the stream. The
differential pins supply padded input vectors (loop bounds are drawn small). This work also fixed a latent
beta bug: reads inside summarized bodies previously got a *fixed* global index — `n · (first byte)` instead
of the sum — contained only because alpha refused the shape.

Read deltas **compose with the linear machinery**: `_split_stream` separates a delta into its stream part
and its rest — `total += a·read_byte() + i` closes as `g(n) + a·Σ input[base..base+n)`, the rest through the
ordinary series and the stream part as `(m (k 8 lo hi) coef)` with a loop-invariant coefficient. A quadratic
stream (`read·read`) refuses.

Subtracting reads compose too: each ℤ-pair component splits its own stream part, so `total -= read_byte()`
puts the `Σ` on the pair's **neg** side — `(k 5 init (p z Σ))` — and `total += a − read_byte()` mixes an
ordinary series on pos with a stream sum on neg (`_component_closed`, identical in both engines).

**Wide reads**: `total += read() + read()` consumes both of the iteration's reads — consecutive reads are
contiguous, so the closed form is just a *wider* sum, `Σ input[base .. base + R·t)`, whose upper end is
exactly the read position's own series closure. An accumulator consuming only *some* of an iteration's reads
would be a **strided** sum and refuses. Reads under down-counters work (the Σ is direction-independent);
counter-dependent rests under a down-counter refuse.

### Branching on data (conditional terms)

An if-diamond on a symbolic guard **forks**: both paths run to completion on copied state (env / registers /
memory / the read position) — no join detection — and the meaning is the conditional term
`(k 9 b then else)` with the boolean `b` one of `(k 10..13 L R)` (`<`, `<=`, `==`, `!=`; `>`/`>=` normalize
by the same swap bc's codegen performs). `absdiff` certifies as `if a<b then b−a else a−b` with ℤ pairs in
both arms; forks nest (`max(a, max(b, c))`), capped at depth 8. Booleans compare over ℤ — sound because the
machine compares the wrapped value *signed*, which agrees with ℤ for |x| < 2⁶³. Reads taken on a path number
consecutively from the fork point, matching the machine's per-path read order. Loops keep priority
(summarize first, fork only when the guard isn't a summarizable loop); branches *inside* loop bodies still
refuse (conditional deltas — the next slice of this mountain).

Comparisons are also first-class **values**: `let b = (x < y)` materializes the boolean constructor itself
(evaluating to the same 0/1 byte the machine's compare idiom writes), flowing through arithmetic, byte
memory, later branches — and even loop deltas (`total += (a < b)` summarizes to `n·[a<b]`, the boolean as
an invariant coefficient). The engines' internal boolean carries RAW sides (a `_term`-coerced side would
break the summarizer's slot matching — found when the basic `n·a` loop fork-bombed under the coercion).

### Buffer copy loops (fill segments)

`while (i<n): byte[base+i] = read_byte()` — the read-n-bytes-into-a-buffer idiom — summarizes to a
**segment** `(base, trip, rdbase)`: `byte[base+j] = input[rdbase+j]` for `j < trip`. A post-loop read at a
concrete offset becomes the conditional term `(k 9 (k 10 j trip) (k 7 rdbase+j) old)` — in-range reads the
copied stream element, out-of-range the prior memory. This is exactly what conditional terms were the
prerequisite for: the segment bound `j < trip` is symbolically undecidable, so the meaning *carries the
decision as a term* instead of needing to make it. Fill stores are recorded as events during the body run
(never written concretely); the summarizer requires exactly one store per iteration at address
`base + counter` (base concrete), value = the iteration's single stream element, up-counting from 0.
Overlapping segments, prior writes in the range, and later stores over a segment all refuse.

**Deliberately out of scope** (each conservatively *refused* — never mis-summarized): scaling recurrences
(`acc = (acc-1)·2`); division; *genuinely* non-linear counter deltas (`i·i`, `i·total`, tetrahedral `Σg`);
ℤ-pair or monus counter *start values*; `word[..]` memory; quadratic streams (`read·read`); strided stream
sums (one-of-many reads); invariant-value fills (`byte[base+i] = c` — only copy loops for now); segment reads at symbolic
offsets; stale reads of rewrite slots; returns inside loop bodies.

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
| `../beta-lang-py/beta_parser.py` | UNTRUSTED: shared Beta source recognition used by the reference and refinement tools; it does not load a compiler backend. |
| `../beta-lang-py/beta_symbolic.py` | UNTRUSTED: symbolically evaluates a Beta source → the source meaning `M`. The source-side dual; reuses the shared `beta_parser` syntax tree. |
| `alpha_refinement_check.py` | The gate driver: derives `C`/`M`, differentially pins each, proves `(= C M)`. Curated samples + three fuzz spaces. |
| `refinement_fuzz_gen.py` | random straight-line arithmetic programs |
| `refinement_loop_gen.py` | random data-dependent counter loops (`<` / `<=`) |
| `refinement_compose_gen.py` | random pre-loop + loop + post-loop compositions |
| `refinement.sh` | builds `check.beta` + `bc`, runs the driver; the lattice step |
| `refinement-samples/*.beta` | curated end-to-end samples (muln, countn, tri, muln_le, …) |
| `../beta-lang-py/symbolic_loop_check.py` + `symbolic-loops.sh` | source-side soundness gate: `beta_symbolic`'s loop summaries pinned to `beta_interp` over an input grid |

## Reference producers (never run in the trusted lineage)

`alpha_ref.py` (the alpha VM in Python) and `beta_interp.py` (the Beta interpreter) are the ground-truth
references the two symbolic evaluators are pinned against. `proof-kernel/prover.py` searches for the equality proof;
`proof-kernel/check.beta` (the trust anchor) validates it.
