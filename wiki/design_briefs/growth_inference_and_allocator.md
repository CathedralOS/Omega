# Design Brief — Static Growth-Bound Inference & the Future Allocator (decision #9)

> **For:** Omega maintainer · **Status:** decision-forcing · **Driver:** retire the
> magic `String` keyword (#66) — replace the unsound 256-byte scratch hack with a
> *discharged theorem* rather than the heap allocator.
>
> Companion to [`allocator_primer.md`](allocator_primer.md) (landscape) and
> [`allocator_story.md`](allocator_story.md) (A1–A5 staged plan). This brief
> answers the primer's **open decision #9** ("automatic size inference — worth it?")
> and the question "detect how much a buffer could grow and avoid the allocator?".
>
> Synthesized from a 14-stream research sweep (RAML/AARA, refinement & sized
> types, abstract-interpretation loop bounds, the string-length decidability
> gradient, region inference, uniqueness/in-place reuse; Zig, Rust, Jai/Odin,
> Ada/SPARK, Vale/Austral, value-semantics, safety-critical, PMR) and an
> adversarial verification pass. **The verification materially corrected the
> headline — see §6; the body below already incorporates it.**

---

## 1. Bottom line up front

**Detecting a max-growth bound so overflow is impossible *by construction* is the
right design, and it is far cheaper than the heap allocator — but it is NOT free
on the interval engine we have today. The split that matters is straight-line vs
loop:**

- **Straight-line / literal concat** (`"a" + "b"`, `x = x + "y"` a fixed number of
  times) — the bound is a **constant** (sum of segment lengths). The existing
  engine handles it; the all-literal case already folds. **No new machinery.**
- **Bounded-loop append** (`for i in 0..n: buf += s`, `n` refined) — the bound is
  `L0 + n·len(s)`, which requires **relating the accumulated length to the loop
  counter** — a *relational* fact (two variables correlated). Omega's interval
  engine is **non-relational** (it tracks each variable independently), so it
  *cannot express this invariant on its own.* This case needs **one of**: (a) a
  small **relational numeric domain** (octagons — the CSSV recipe), or (b) a
  **blessed loop-append invariant axiom** supplied to the engine (`len ≤ iters ·
  elem_len`), reusing the #60–#64 fact-catalog pattern. Either is modest and
  vastly short of a string solver or amortized-potential system — but it is a real
  (small) extension, not "the engine already does it."
- **Input-refined data-dependent** (`buf += input`, `input.len ≤ N`) — tractable
  *iff* the source carries the `N` refinement **and** the per-element length is a
  *constant coefficient*. If both the iteration count `n` and the element length
  `len(s)` are symbolic, `n·len(s)` is a **product of two variables** — outside
  linear arithmetic, undecidable in general. So data-dependent loops need at least
  one side pinned to a constant/declared bound.
- **Unbounded / nonlinear** (`while cond: buf += s`; `len *= k`; parse-back into
  length) — **provably out of reach** (Ganesh–Berzish 2016: word equations +
  length + string↔number is *undecidable*; word-equations-with-length alone has
  been open >50 years). Here the analysis **widens to ⊤ and forces the next rung
  up** — declared bound, or `Vec` + allocator with a fallible/abort escape. The
  unbounded case is *rejected at the low rung*, never silently truncated as the
  256-byte hack does today.

**The future allocator is the settled rung lattice** (static → stack → fixed-arena
→ proven-`Region` → bounded-heap → unrestricted) **with the allocator as a
capability and remaining-capacity as an interval-tracked quantity.** The whole
field (Zig, Ada/SPARK, Jai/Odin, Vale/Austral, Verona, PMR) validates every rung
*ergonomically* but proves *nothing* — every existing bound is a runtime check or
a prohibition. Omega's genuinely novel, defensible claim is **no-OOM as a
discharged theorem on the same engine that discharges array-index bounds** — the
unification the safety-critical industry assembles today from 3–4 tools that don't
share a semantic model. Concentrate the novelty work on **rungs 3–4** (proven
`Region` + global peak-accounting) — the exact piece Ada/SPARK *deferred*.

---

## 2. Static growth-bound inference: the feasibility verdict

### 2.1 The decidability gradient (sharp, and it maps onto the rungs)

| Tier | Form | Length transfer | Tractable? | What it needs |
|---|---|---|---|---|
| **Literal / straight-line concat** | `"a"+"b"`, fixed # of `x=x+"y"` | `len = Σ const` | trivially | **today** (folds) |
| **Bounded loop, const element** | `0..n: buf+="x"`, `n≤N` | `len ≤ L0 + n·c` | yes — *linear* | **relational invariant** `len≤iters·c` (octagon OR blessed axiom) |
| **Bounded loop, refined element** | `0..n: buf+=s`, `n≤N`, `s.len≤c` | `len ≤ L0 + N·c` | yes — if a side is constant | refinement **+** const coefficient |
| **Symbolic × symbolic** | `n` and `len(s)` both free | `n·len(s)` | **NO** (nonlinear) | pin one to a constant, else reject |
| **Unbounded loop** | `while cond: buf+=s` | `len → [k, +∞)` | no finite bound | **widen to ⊤ → reject / rung up** |
| **Nonlinear / parse-back** | `len *= k`; `len += parse(s)` | undecidable | **NO** (Ganesh–Berzish) | **reject at low rung** |

The boundary is exactly the **linear/nonlinear** and **bounded/unbounded**
frontier. The principled signal at the frontier is *sound widening*: for
`while(cond) x = "0"+x+"1"` a correct analyzer **must** drive the length's upper
bound to `+∞` (it cannot fabricate a finite bound for a genuinely unbounded loop).
That widening *is* the automatic "needs a refinement or a higher rung" trigger.

### 2.2 What the existing engine discharges — and the one honest gap

Omega's engine (`omega-typed-trees-to-checked-trees`: `semantic.rs`,
`flow/transfers.rs`, `field_domains.rs`) is **non-relational interval abstract
interpretation** in the Liquid-Types lineage: exact-by-default ints, `a..b`
refinements, flow-sensitive narrowing via interval merge, dominating-`.len`-guard
narrowing for `s[i]`, return-range inference (`infer_return_interval`, #64). It
already discharges `i < len`-style index queries — *non-relational* ones, where
each fact is about a single place.

The growth-bound query for a **loop** is different in kind. To prove `final_len ≤
cap` for `for i in 0..n: buf += s` the engine must hold the **loop invariant
`len = (iterations so far) · len(s)`** — a *relation between two variables*
(`len` and the counter). A non-relational interval domain represents `len ∈
[0, ?]` and `i ∈ [0, n]` *independently* and **loses the correlation** — exactly
why CSSV (the canonical "statically catch all C buffer overflows" tool, which
*does* reduce string safety to a linear integer program over per-buffer
`strlen`/`alloc`/`offset` ghosts) discharges that program with the **relational
polyhedra** domain plus procedure contracts plus a points-to pre-pass — **not**
with intervals. **The over-claim to avoid:** "Omega's interval narrowing already
discharges growth bounds." It discharges the *straight-line* fragment (constant
bounds); the *loop* fragment needs relational reasoning.

Two viable, cheap ways to close that gap (pick per appetite; they compose):

1. **Bless the loop-append invariant as a supplied measure/axiom.** Treat the
   `{ptr,len}` carrier's `len` as a one-argument structural **measure**, and add
   `len(buf++rhs) = len(buf) + len(rhs)` and the loop form `len ≤ iters · elem_len`
   as **hand-proved built-in laws** (F\* `SMTPat` / Liquid-Haskell-measure style),
   reusing the #60–#64 fact catalog. The engine then *applies* a blessed linear
   law instead of *synthesizing* an induction (which an interval/SMT engine will
   **not** do — `len(xs++ys)=len xs+len ys` is not a measure in LH and is a stated
   recursive lemma in F\*). This is the **prover-light** path and the recommended
   first move: it keeps no-OOM a *linear* theorem and adds no new abstract domain.
2. **Add octagons (a bounded relational domain).** Octagons express `±x ± y ≤ c`
   — enough for `len − iters·c ≤ 0` — at near-interval cost (the workhorse below
   polyhedra). This is a genuine but well-trodden extension if blessed invariants
   prove too narrow.

Either way the resulting entailment — *"is `L0 + n·c ≤ cap` for all `n ∈
[0,N]`?"* — is **Presburger (decidable linear-integer) when the coefficient `c` is
a constant.** When `c = len(s)` is symbolic, `n·c` is bilinear (a product of two
free variables), which is **not** Presburger and undecidable in general — so the
element length must be pinned to a constant or a declared `≤ c` bound.

### 2.3 What needs a declared input refinement (annotation, not inference — and that's right)

Any append of a value whose length isn't statically known requires the source to
carry `s.len ≤ N`. This is **annotation, deliberately** — inference cannot recover
a caller's fact, and the alternatives are worse: Liquid Haskell auto-infers loop
invariants but needs the append law *supplied*; Idris/Agda size-indexed `Vect
(n+m)` gives the concat law *for free in the type* but **forces explicit
`rewrite`/`Eq` proof terms** for the surrounding API (`n+0=n`, `n+m=m+n` are only
*propositional*, not definitional) — the wrong cost profile for a prover-light
language. **Avoid `Vect`.** Bless the laws (path 1) and accept the one declared
`≤ N` per unbounded source.

### 2.4 What must degrade to a fallible/abort escape

At the ⊤ frontier the low rungs **reject** and the program moves to `Vec<u8>` +
allocator capability where the append is **fallible** (out-of-capacity result) or
**aborts** under a declared budget. Widening-to-⊤ is the *automatic* trigger.

### 2.5 What to BORROW vs AVOID from the research

- **BORROW (ideas, not machinery) from RAML/AARA.** Its soundness theorem bounds
  the **high-water-mark (peak)** of resource use — *exactly* Omega's peak-not-net
  property (verified against Kahn–Hoffmann 2021) — so compositional **peak** bounds
  are a *known-achievable shape*, not aspiration. Steal Pastis-style **weakening
  hints** (sound user-supplied bounds when inference gives up) as the
  annotation-escape pattern, and heed the **certified-LP lesson** (LP/SMT backends
  silently return unsound numerics) — reinforces self-checking provenance if Omega
  ever leans on SMT.
- **AVOID making AARA the foundation.** It is **polynomial-only** (binomial-
  coefficient basis), fails on **difference-of-sizes** and **semantic-value**
  properties (`"RaML cannot reason about program values"`). On mutable cells it
  requires **invariant** potential and the ordinary dereference yields *no usable*
  potential; recovering a mutable buffer's length↔inputs relation needs the
  Lichtman–Hoffmann 2017 **manual `swap` discipline** (swap-out/share/swap-in) —
  *possible but awkward*, the wrong ergonomics for automatic buffer bounds. The
  interval/refinement path is strictly cheaper here and already shipped. Reserve an
  amortized-credit refinement as an **optional opt-in rung** for amortized growth
  (dynamic-array doubling) only.
- **AVOID whole-program region/size inference (Tofte-Talpin / MLKit).** Documented
  **unbounded, program-dependent space leak** — per MLton, "the global region's
  size will grow to an unbounded multiple of the live data size" (ML Kit benchmarks
  show 2×–150× blowups without GC) — *categorically incompatible* with a no-OOM
  proof. **Nuance from verification:** the leak comes from region lifetimes tied to
  *static scope* ("no matter what region system you use"), not from inference being
  whole-program per se. Keep the lifetime layer **local + signature-annotated**
  (the **Cyclone** recipe — Grossman/Morrisett et al., PLDI 2002 — explicit
  annotations + default annotations + *local* inference for separate compilation).
  *Do not* co-attribute this to Gay/Aiken's `RC` (PLDI 2001), which is a *different,
  dynamic* reference-counting approach with no a-priori lifetime restriction —
  Omega's `'name`+elision lifetime work is the Cyclone lineage.
- **AVOID general SMT string theory (T_SLIA / Z3str3 / CVC5-strings).** Decidability
  open; solvers don't guarantee termination "even for decidable theories." Stay
  strictly inside the decidable linear-*length* fragment — never reason about
  string *contents*.
- **BORROW uniqueness/in-place reuse later.** Perceus (Koka) / Lean reset-reuse /
  FIP show that proving unique ownership lets the compiler reuse a buffer's storage
  with **zero alloc** — directly relevant to Omega's in-place `buf = buf + "x"`
  (an affine/linear `buf` can be appended in place). A natural rung-2 optimization,
  not foundational.

### 2.6 The concrete #66 mechanism

Replace the unsound `DEFAULT_RUNTIME_TEXT_OUTPUT_BUFFER_CAPACITY = 256`
(`omega-runtime-text/src/planning.rs:15` — verified: every owned generated-text
write gets a flat 256-byte buffer, `.max()`-merged across uses, **no growth
analysis, no runtime guard**; an emitted string > 256 bytes writes past the slot)
with a **proven byte bound that sizes the carrier**:

1. At each concat/append site (planned in `omega-runtime-text/src/planning.rs`),
   compute a symbolic upper bound `B` on the final `len` — straight-line by the
   blessed concat law, loops by the blessed loop-append invariant of §2.2.
2. If `B` is a static constant or `≤ N` for a declared input refinement, **size a
   `FixedVec<u8, B>`** (heap-free, the sound bounded carrier).
3. Each append is a `push`; `push`'s capacity obligation `self.length <
   self.items.len` is **already a compile-time obligation** (FixedVec ships;
   `collections/fixed_vec_push_without_room` is in `ACTIVE_FAIL_CANARIES`, rejected
   with *"cannot prove requires contract for call push"* — verified). The
   growth-bound proof discharges that obligation for the **whole chain** at once.
   **Overflow is impossible by construction: no runtime trap, no fallible arm on
   the proven path.**
4. If `B` widens to ⊤, reject at this rung → `Vec<u8>` + allocator (deferred rung).

**Soundness invariant (the #40 trap, restated):** never ship the growth-bound
*narrowing* without its *write-enforcement*. Every length-affecting write — the
construction site **and** every loop-body append — must be capacity-checked against
the declared carrier, mirroring the construction-1c + assignment-#63 enforcement
already in place for range-refined fields.

> **Caveat (verification):** FixedVec's *generic* body is blocked on generic-machine
> instantiation; the shipped fail-canary uses a concrete `FixedVecI32x4`. The
> generic surface type-checks, but a `FixedVec<u8, B>` with a *computed* `B` may hit
> the same generic-instantiation frontier (primer open decision #8). Resolve or
> sidestep (monomorphize per call site, as FixedVec does today) before step 1 ships.

---

## 3. Allocator landscape, refreshed — one decisive lesson each

- **Zig** — *fallible-by-default allocation is ergonomically viable at stdlib
  scale*, validating allocator-as-parameter. But **every bound is a runtime
  bump-pointer check, never a static theorem**; the allocator type carries no
  capacity/region/lifetime; `checkAllAllocationFailures` (its strongest substitute
  for a proof) needs determinism and gives no compile-time coverage. *Omega
  exceeds Zig by turning the FixedBuffer bound into a discharged obligation so the
  OOM arm provably can't be taken — and needn't appear in the signature.*
- **Rust** — *the allocator generic is viral* (`Allocator`/`Vec<T,A>` unstable ~9
  yrs for exactly this); `GlobalAlloc` **aborts** on OOM, fallibility is
  `try_reserve` only. *Lesson: allocator as a **capability**, not a viral generic —
  capabilities compose; viral generics colour every enclosing type.* (`heapless`'s
  `Vec<T,N>` is the direct FixedVec analog, runtime-checked.)
- **Jai / Odin** — *ambient-context allocation is ergonomic but every footgun is an
  unproven obligation*: dangling-after-`temp`-reset, fixed-arena overflow (Odin
  patched it with a runtime `panic_allocator`), lifetime mismatch. Odin's static
  virtual arena (reserve big, commit-on-demand, stable base) is a clean
  **peak-not-net** substrate. *Omega may keep ambient ergonomics only if the
  defaulted capability stays **visible to the checker** (effect-row) — quiet, never
  invisible.*
- **Ada / SPARK** — *the closest prior art, and it draws the exact boundary Omega
  must cross.* GNATprove discharges **every local value/range run-time error EXCEPT
  `Storage_Error`** — the single named residual. **Verification corrections:**
  `Storage_Error` has **three** sources — *primary stack, secondary stack, and
  heap* — not two; and SPARK's reason is that these are **whole-program resource
  properties outside modular deductive verification** (it *assumes* allocations
  never fail and delegates sizing to GNATstack / `-fstack-check` /
  `No_Secondary_Stack`), which is consistent with "the allocator was never modeled
  as a finite capacity in the logic." Bounded containers reach no-OOM **by
  elimination** (no heap; `Capacity_Error` as a provable precondition). *Omega aims
  to be first to no-OOM with a real bounded/growable allocator **proven not to
  fail** — strictly harder than SPARK's "ban the heap."*
- **Vale / Austral** — *temporal safety without GC is achievable, but the
  region-attached-allocator rung is the hard part, not a free consequence of
  borrowing.* Vale's generational references are **runtime**-checked (8 B/alloc);
  its zero-cost region layer stalled unfinished. Austral proves
  **capabilities-as-linear-values in a < 600-line checker** — Omega's enforcement
  need not be Rust-scale. *Neither ships a no-OOM theorem or mechanized soundness.*
- **MVS / value semantics (Hylo/Val, Swift, Mojo)** — *exclusivity is what enables
  the allocation theorem*: no aliasing ⇒ stack/arena without RC/GC. Omega's affine
  model **is** MVS exclusive access; Hylo's `inout` reference-returning subscripts
  model the `{ptr,len}` slice ABI and make borrowed-place narrowing sound. *Limit:
  pure MVS can't store mutable refs/graphs — needs the proof-gated aliasing escape
  (SharedRegion).*
- **Safety-critical (NASA P10, MISRA, DO-178C, ARINC-653, seL4)** — *the whole
  industry accepts Omega's premise (only static/bounded allocation is provable) but
  reaches it by **prohibition** + a tool patchwork.* Holzmann's own framing: the
  stack bound is "derived statically" **only because** recursion is banned (P10 R1)
  and every loop has a statically provable upper bound (R2) — **the memory theorem
  is parasitic on the bounded-iteration theorem** (this is precisely §2's
  bounded-loop requirement). seL4 makes allocation an explicit capability-tracked
  watermark op with no kernel heap — strong precedent for allocator-as-capability —
  but proves spec-refinement, *not* an app-level no-OOM theorem. **Verification
  nuance:** the tools aren't *uniformly* disconnected — AbsInt's `aiT` (WCET) and
  `StackAnalyzer` share the `a³`/CRL2 framework. The genuine fragmentation is
  between **deductive correctness proof** (GNATprove) and **quantitative resource
  bounding** (WCET/stack), which share no semantic model — that's the seam Omega
  closes. (GNATstack's static stack bound itself needs **four** conditions — no
  recursion, no dynamic frames, **no unresolved indirect/dispatching calls**,
  whole-program access — so Omega's *dyn-dispatch* sites need call targets resolved
  before the existing stack capability applies cleanly.)
- **C++17 PMR / production allocators** — *separate the three "multi-heap" wins.*
  **Lifetime** (bulk-free cohort) = the fixed-arena/`Region` rung. **Locality**
  (per-core hot memory: mimalloc/snmalloc/tcmalloc/jemalloc — no universal winner
  per warehouse-scale studies) = an **implementation detail below the language**,
  a pluggable backend under the bounded-heap rung, **never a type concept**.
  **Isolation** (refs can't escape a heap) = the *only* one that must be a prover
  concern = the proven-`Region` rung (Verona's closest prior art, via linear `iso`
  not an interval engine). *AVOID PMR's anti-pattern — allocator as a **runtime
  value** (`memory_resource*`, vtable-per-alloc) erases which heap an object's
  storage came from at the type level; Omega's "domains stay bound to storage, no
  storage-less domains" is the correct opposite stance.*

**Where Omega already exceeds the field:** capabilities + effects + `'name`
lifetimes + interval refinements + FixedVec's compile-time push obligation + ZII
are all shipped — ahead of Zig/Jai/Odin (unproven) and orthogonal to Rust (no
viral generic). **Where it lags:** global peak-accounting (the work Ada
*deferred*), the proven-`Region` rung (Vale stalled here), and a *mechanized*
soundness proof (no one has one yet).

---

## 4. What a future-language allocator looks like (the synthesis)

| Rung | Omega | Field analog | Proven? |
|---|---|---|---|
| 0 static / ZII | length is a literal | P10 init-only; Ada `[u8;N]` | trivially |
| 1 stack | sized frames, no recursion | GNATstack subset | yes (integration; needs dyn-call resolution) |
| 2 fixed-arena | FixedVec, push obligation | Zig FixedBuffer / Ada bounded / PMR monotonic | **ships — Omega's novelty: *static*, not runtime** |
| 3 proven-Region | `alloc(r,n)` obligation `n ≤ remaining`, **infallible after proof** | Cyclone region + Odin watermark; SPARK pool (runtime exception) | **the frontier — SPARK's `Storage_Error` residual → a theorem** |
| 4 bounded heap | budgeted, fallible/abort escape | jemalloc/mimalloc *under* the rung | partial |
| 5 unrestricted | — | GC / `GlobalAlloc` abort | no |

**Genuinely novel (the defensible claim):** (1) **no-OOM as a *discharged
theorem*** rather than a prohibition (MISRA/DO-178), an unproven runtime check
(Zig/Jai/Odin), or a residual exception (SPARK `Storage_Error`); (2) **growth,
capacity, *and* index bounds discharged on the *same* refinement engine** — the
unification the safety-critical world assembles from a linker map + `-fstack-usage`
+ a WCET tool + GNATprove, none sharing a model; (3) **peak-not-net high-water-mark
budgeting** with region-reset as a disjointness proof (the AARA peak-soundness
shape, on intervals/octagons not potential).

**Borrowed:** allocator-as-capability (seL4 untyped+Retype, Austral linear
capabilities); arena/bump as the cheap rung (Zig, Jai/Odin, PMR monotonic); local
+ signature lifetime discipline, *never* whole-program inference (Cyclone);
exclusivity-enables-the-theorem (MVS/Hylo); pluggable locality backend *below* the
language (mimalloc/snmalloc); weakening/rewrite annotation hints (Pastis); in-place
reuse via uniqueness (Perceus/FIP) as a later rung-2 optimization.

---

## 5. Recommendation + ordered next steps

**Recommendation: YES — build static growth-bound inference for the bounded-string
case, as a *linear length-arithmetic* theorem. Be honest that the loop case needs
EITHER a blessed loop-append invariant axiom OR a small octagon domain — not the
bare non-relational interval engine. Do NOT build AARA, a string solver,
whole-program region inference, or size-indexed `Vect`.** The future allocator is
the settled rung lattice with allocator-as-capability; concentrate novelty on rungs
3–4 (proven `Region` + global peak-accounting), the piece Ada deferred.

Ordered, tied to the lattice and the #66 unblock:

1. **(#66, rung 2 — ships first) Straight-line concat → proven bound.** The
   all-literal and fixed-count cases fold to a constant today; size a
   `FixedVec<u8, B>` from it, discharge the existing push obligation for the whole
   chain, delete the 256-byte hack. Canaries: all-literal chain (folds), and a
   *negative* canary proving the proven path has **no fallible arm**. Enforce every
   length-affecting write (#40/#63). Resolve the FixedVec generic-`B` instantiation
   question (primer #8) first.
2. **(rung 2) Bless concat/push/split as built-in measures with hand-proved linear
   laws**, reusing #60–#64. This is the difference between "engine discovers the
   law" (it won't) and "engine applies a blessed axiom" (it will) — and it is what
   unlocks the **bounded-loop** case without a new abstract domain. Add canaries:
   input-refined loop (sizes from `N`), unbounded loop (must *reject*, widening to
   ⊤), symbolic×symbolic (must *reject* unless a side is pinned).
3. **(rung 2, optional) Add octagons** only if blessed invariants prove too narrow
   in practice — the relational fallback CSSV validates.
4. **(rung 3) Proven `Region`: `alloc(r,n)` obligation `n ≤ remaining`, infallible
   after proof.** SSA-thread the remaining-bytes interval through the *same*
   arithmetic-domain merge (resolved ergonomics R3). SPARK's heap `Storage_Error`
   → a theorem — the headline novelty. Keep the handle affine/non-escaping
   (Omega's body-escape analysis already rejects `-> &local`).
5. **(rung 3, peak-accounting) Peak-not-net high-water-mark summation** — the
   global obligation Ada deferred. Per-site proofs are local; `total live ≤ budget`
   is global; region-reset = disjointness proof. Hardest and most defensible. If it
   doesn't scale, fall back to per-region budgets with a declared envelope — never
   silent over-approximation.
6. **(rung 4) Bounded heap** with a pluggable locality backend (mimalloc/snmalloc
   *under* the rung, invisible to the prover) and the fallible/abort escape for
   everything that widened to ⊤.

### Residual annotation burden (honest)

- **Input-refined appends need a declared `s.len ≤ N`** on the source — inference
  cannot recover a caller fact. Minimal and exactly where it belongs.
- **Relational length laws are blessed axioms, not discovered** — a one-time
  hand-proof per length-transformer, not per program.
- **Allocator-capability visibility** adds signature noise (the Zig/PMR critique);
  mitigate via the resolved R1–R5 elision/ambient-tracking so the common case is
  quiet but never invisible to the checker.

### Failure modes (honest)

- **Nonlinear growth, symbolic×symbolic, and string→int→length feedback are
  undecidable** (Ganesh–Berzish) — *reject* at low rungs, drop to `Vec` + allocator;
  do not attempt to prove them.
- **The bounded-loop case is not free on the current engine** — it needs blessed
  invariants or octagons (§2.2). Budget for that small extension; don't promise
  zero work.
- **Global peak-summation across many sites** is where the real difficulty lives
  (the work SPARK avoided).
- **Fragmentation is sidestepped only *within* a region** by the arena model —
  cross-region fragmentation is not addressed and should not be claimed.
- **The no-OOM theorem is conditional** on the trust root (the Alpha seed /
  self-building lattice) and on enforced write/construction bounds — state this
  explicitly, as seL4 states its hardware/boot/asm assumptions.

---

## 6. What the adversarial-verification pass changed (provenance)

Of 16 load-bearing claims, the skeptic pass returned **8 supported, 7 partly, 0
refuted, 1 supported-with-minor-precision.** The material corrections, all folded
into the body above:

- **The single most important fix:** the brief originally said Omega's *existing
  interval engine already discharges* growth bounds. **It does not for loops** —
  the interval domain is **non-relational**, and a bounded-loop bound is a
  *relation* between `len` and the loop counter. CSSV (the cited precedent)
  achieves it with **polyhedra + contracts + points-to**, not intervals. The
  corrected story: straight-line is free; loops need a blessed invariant axiom or
  octagons (§2.2). *This is the key honest caveat — the idea works, but it is a
  small extension, not magic on what's shipped.*
- **Bounded-loop decidability** is Presburger only when the element length is a
  **constant coefficient**; `n·len(s)` with both symbolic is bilinear and
  undecidable (§2.1–2.2).
- **AARA** uses **invariant** (not zero) potential on mutable cells, and the 2017
  reference/array extension *can* bound mutable-cell-dependent sizes via a manual
  `swap` discipline — so "discards the relation entirely" was wrong; the real
  friction is the swap burden (§2.5). Still: avoid AARA as the base.
- **SPARK `Storage_Error` has three sources** (primary stack, secondary stack,
  heap), not two; and SPARK's reason is "whole-program resource properties outside
  modular deductive verification," not literally "never modeled the allocator" (§3).
- **Region space leak / Cyclone attribution:** the leak is from region lifetimes
  tied to static scope (not whole-program inference per se); cite **Cyclone**
  (Grossman/Morrisett) for the local+annotated model and **do not** co-attribute it
  to **Gay/Aiken's `RC`** (a different, dynamic refcounting approach) (§2.5).
- **GNATstack** needs four conditions (incl. resolved indirect calls) — relevant to
  Omega's dyn-dispatch (§3).
- **Safety-critical tools aren't uniformly disconnected** — `aiT`+`StackAnalyzer`
  share a framework; the real seam is deductive-proof vs resource-bounding (§3).

Fully supported as stated: the Ganesh–Berzish undecidability + the >50-yr-open
word-equations-with-length problem; sound widening-to-⊤ for unbounded loops; AARA
polynomial-only / value-blind; relational length laws are non-synthesizable axioms;
`Vect` forces rewrite proofs; the 256-byte hack is real and unsound; FixedVec's
compile-time push obligation; AARA's peak-soundness shape; PMR erases heap identity.
