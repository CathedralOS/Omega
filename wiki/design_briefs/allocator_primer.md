# Allocator Design for Omega — Primer

> Landscape + rationale to prime the allocator decision. The concrete staged
> plan + decisions (A1–A5) live in [`allocator_story.md`](allocator_story.md);
> this is the "why / what does everyone else do / what fits us" companion.
> Synthesized from a 6-facet research sweep (Rust, Zig/Jai/Odin, safety-critical,
> regions/multi-heap, proofs/guarantees, Omega-fit); implementation claims were
> checked against the then-live corpus and must be rechecked as the Arena slice
> lands.

The durable model is an explicit bounded `Arena` capability with dependent
resource contracts; reaching an allocation boundary contributes its
boundary-trait service identity. Quantitative `Alloc<Peak, Retained>` rows
remain deferred to the resource algebra.

**Where Omega is today.** No heap, no allocator. Storage is inline (`[T;N]`,
struct fields), bounded (`FixedVec<T,N>` — `push` is a *compile-time* proof
obligation `len < N`, never a runtime trap), or borrowed (`{ptr,len}` slice
descriptors). Zero-expressibility is a representation preference, not a promise
that zero establishes every value; domains and construction gate authority or
validity when zero would forge either.
The allocator is **designed but unbuilt**: `allocator_story.md` specifies an
`Arena` as a bounded lifetime-scoped capability, backed by an Extent or admitted
provider, with explicit allocation authority and resource contracts. Concretely
blocked: `Vec<u8> in Utf8` (owned/growable text) and copy-out wire decode.
Fixed-capacity console input is no longer allocator-gated:
`Console::read_line(&mut [u8])` specializes against the caller's concrete
bounded carrier.

## What other languages do (and the one lesson each)
| Stack | Model | The catch |
|---|---|---|
| **Rust** | One stable global allocator singleton; per-container `Allocator` as a generic *type param* (`Vec<T,A>`), unstable ~9 yrs | Allocator-as-viral-generic infects every enclosing struct; abort-on-OOM default (`try_reserve` is a partial, reserve-only retrofit) |
| **Zig** | Allocator is an *explicit value* threaded into every allocating fn; fallible-by-default (`error{OutOfMemory}!T`) | Maximally honest + testable, but signature noise ("allocator coloring"); ownership is convention, not proof |
| **Jai / Odin** | Ambient `context.allocator` + per-frame `temp` arena, swapped lexically | Clean signatures, retargets 3rd-party allocs per scope — but allocation is invisible thread-local state; temp-reset use-after-free |
| **C++17 PMR** | Allocator = *runtime value* (`memory_resource*`); one container type, swappable strategy | Fixes the type-param pain, but a vtable call per alloc and the **type system tracks nothing** about which heap a value lives in |
| **Safety-critical** | No heap after init; many fixed-size *pools* + per-cycle arenas; bounds proven by external tools | Deterministic, fragmentation-free, certifiable — at worst-case-sizing waste + rigidity |
| **Regions / multi-heap** | Lifetime as a bulk property (Tofte-Talpin → Cyclone `*'r` → Rust `Vec<T,&'a Bump>`); per-subsystem/per-core heaps; seL4 untyped+Retype | O(1) bulk free + isolation/accounting; but cross-heap refs dangle, no individual free, per-heap fragmentation |

Two convergences worth internalizing: **everyone reaches for a bump/arena** for
scoped data, and **an allocator is functionally a capability** (unforgeable
authority to obtain memory) — "pass the allocator" *is* capability-passing, "the
ambient context" *is* a dynamically-scoped effect.

## Design pressures

- **Multiple heaps.** For an OS, they are largely inevitable — but “multi-heap”
  bundles three independent wins: lifetime (per-request/per-frame bulk reset),
  contention/locality (per-core/NUMA heaps — jemalloc/tcmalloc/mimalloc, an
  implementation detail), and isolation/accounting (per-component quotas — seL4,
  Zircon VMOs). Treat them separately; only lifetime and isolation belong in the
  type system. Per-core/NUMA is a backend detail that should NOT.
- **Avionics and robotics.** The whole field already agrees: **no
  dynamic heap after init.** Three reasons each fatal to certification —
  malloc/free have unbounded WCET (breaks scheduling), general heaps fragment (a
  request fails after long uptime despite free bytes), and OOM is unprovable.
  Enforced today by *coding-standard bans* (MISRA 21.3, NASA Power-of-10 R3,
  ARINC-653 init-only), **not by any language.**
- **Pre-allocation with a no-OOM proof.** This *is*
  hard-real-time practice — static budgets, fixed pools, per-cycle arenas. But
  the "proof" is a patchwork: linker budgets + WCET tools (aiT/RapiTime) +
  stack-depth analysis + partial formal proof (SPARK/GNATprove). **No language
  unifies it.** Tellingly, SPARK proves absence of *all* run-time errors
  **except `Storage_Error` (memory exhaustion)** — its answer there is "don't
  allocate dynamically." **That residual is exactly Omega's opening.**
- **No allocator.** The no-heap model
  (Rust heapless, Ada bounded containers, Omega's `FixedVec`) makes OOM
  *unrepresentable*: strongest guarantee, lowest proof burden. Cost: every
  variable-length thing carries a compile-time capacity `N`, so you
  worst-case-provision everywhere and can't size to actual data. **This is
  already Omega's default.**

*Two caveats for any "no OOM" claim:* stack depth is residual dynamic memory
even with no heap (must bound recursion separately), and a general free-list
allocator's "fits remaining bytes" is necessary-but-not-sufficient under
fragmentation — **arena/pool discipline (monotone subtraction) is what makes the
budget arithmetic sound.**

## What fits Omega (grounded in its actual subsystems)
The thesis: an allocator **is** a capability; a bounded arena **is** a range
refinement; "no OOM" **is** a theorem discharged on the *same interval engine*
Omega already runs for arithmetic domains. No net-new verification subsystem —
it reuses capabilities/effects (ch19, shipped), the interval/refinement prover,
lifetimes (decision 15), and the `{ptr,len}` descriptor ABI.

- **A — Stay heap-free (the floor).** `FixedVec<T,N>`/`[T;N]` ship today; OOM
  impossible by construction. *Gap to close first:* generic-machine
  instantiation (FixedVec's real bodies are pinned concrete to i32/N=4; generic
  `data` whose layout depends on `T` fails layout).
- **B — `Arena<'a>` with a *proven capacity refinement* (the
  differentiator).** Arena construction establishes a `remaining` fact;
  `allocate(a,n)` carries obligation `n <= remaining`, postcondition
  `remaining' = remaining - n`; the Arena handle is threaded **affinely** so
  the budget can't be double-spent. `alloc` is **infallible after proof**
  (returns a bare handle, no Result) — SPARK's `Storage_Error` residual turned
  into a discharged theorem. *Cost:* data-dependent sizes need worst-case
  *input* refinements (`input: &[u8] [len <= N]`); unboundable sites degrade
  visibly.
- **C — `Vec<'a,T>` borrowing an Arena, capacity fixed at `with_capacity`, NO
  growth (allocator-story stage 2; smallest viable unblock).** Bind one
  `Allocation` provider (host malloc), extend the descriptor to `{ptr,len,cap}`
  under its existing owner `omega-runtime-abi`, lower `core/vec.omg` ops through
  `Allocation` + the working `vec_views` provider, wire drops. Unblocks owned
  text/decode/`read_line` with no realloc — reuses FixedVec's
  proof-obligated-capacity model with a runtime capacity measure.
- **D — Demote unprovable sites to an explicit fallible allocation outcome or
  abort reach.** Keeps unboundable inputs *visible*, not unsound (mirrors SPARK's
  verdict). Risk: over-use fragments the fallibility story.

**Three non-negotiables when building B/C:** (1) enforce *every* budget
decrement at the alloc site (the prior "trust a declared range without enforcing
writes" trap that bit field-domain narrowing); (2) keep zero `{ptr:0,len:0,cap:0}`
a valid empty ZII inhabitant — the empty byte sequence already satisfies the
Utf8 domain gate; (3) oracle/dungeon-gate the lowering (allocator codegen is a
silent-miscompile risk class).

## Recommendation
**Ship the ladder, not a heap.** Keep model A (`FixedVec`/`[T;N]`) as the
permanent default — it is the safety-critical answer and already works. Add
model B (`Arena<'a>` whose contract carries a proven `remaining` interval) as the
differentiator no mainstream stack offers. Concretely, **build allocator-story
stage 2 first** (Arena provider + fixed-capacity `Vec<'a,T>`, no growth): least
machinery, unblocks the real corpus. Allocation-provider reach is inferred and
reported from day one so allocation-free code is mechanically visible. Demote genuinely-unbounded sites
to fallible/abort (D) rather than weakening the proven core. Defer growth,
pluggable allocators, and `try_push` to stage 3 until demand is real. Optionally
layer RAML/AARA-style automatic size inference later to cut annotation burden.

## The rung lattice, the bootstrap, and downstream opt-in
The rungs form a **partial order (lattice), not a total ladder** — memory
(static → stack → fixed-capacity bump allocation → proof-bounded Arena →
bounded general allocator → unrestricted allocator) and
concurrency (static → awaitable) are separate axes with no forced order between
them. A program or build target **picks a fixed point** and the compiler enforces
that point's obligations — turning DO-178C/MISRA/Power-of-Ten constraints from
coding-standard prose into *prover-enforced* bounds. Code at rung 3 need never
have implemented rungs 0–2; the compiler carries them.

**Bootstrap connection (our synthesis; `cathedral_alignment.md` keeps
Thompson-resistance an aspirational Tier-2 TBD, skipped until self-host).** For a
*self-hosting proof-carrying* compiler the usual two ladders fuse: a feature is
usable on the compiler's **own source** only once *both* its lowering AND its
proof machinery exist in the running stage. So the **seed** must restrict its own
source to rungs 0–2 (heap-free / fixed-capacity bump allocation, no prover
needed) — rung 3 (proof-bounded Arena) is un-admittable until the interval engine
is self-hostable. Caveats from
precedent: (a) this binds the seed's *source*, not the running binary at every
stage — real bootstraps (Guix hex0→Mes→tcc→gcc, GCC 3-stage, mrustc) route
*through* a mature host language and allocate freely from the first C-subset rung;
(b) Thompson-resistance is **orthogonal** — Diverse Double-Compilation closes
trusting-trust, the minimal seed buys trust-base shrinkage + per-rung
auditability, and the proof-kernel is a *third* independent ladder. Keep all
three claims distinct.

## Affine-handle ergonomics — recommended design (decision #2)
The borrow-backed affine `Arena<'a>` handle is *positional in the move-graph*, so naïvely it's
worse than Zig's allocator coloring (signature tax + hand-threaded version chain +
higher-order virality). Omega is the rare language with all three levers to
collapse the noise — layer them:
- **R1 elide `'a`** (Rust/Cyclone) when one Arena is in scope. Reject
  Tofte-Talpin whole-program inference — its failure mode is a *space leak*,
  poison for a no-OOM proof.
- **R2 tracked capability delivery** (Jai delivery + Scala/Austral tracking):
  a selected Arena may be threaded to leaf allocation without becoming ambient
  authority; intermediate summaries retain its resource/reach requirement.
- **R3 `inout`/consume passing mode** (Val/Hylo) not rebind-and-return — the
  compiler reconstructs consume-and-rebind and **SSA-threads the remaining-bytes
  interval using the SAME merge/narrowing the arithmetic-domain engine already
  does.** Biggest single win.
- **R4 effect-row inference** so higher-order combinators can eventually remain
  polymorphic over their callee's allocation-service reach (deferred until that
  customer exists).
- **R5 explicit outcome routing** at genuinely fallible boundaries; no `?`
  syntax is assumed.

**Irreducible residuals** (must stay explicit — they ARE the theorem's spec):
declare the budget bound; disambiguate when more than one Arena is live; mark each
**split/fan-out** (a sequential thread can be hidden, a partition cannot); mark
each fallible escape hatch; state `outlives` at escape.

## Settled accounting law

Track peak live bytes, not net allocation. A persistent Arena does not regain
capacity merely because an object becomes unreachable. A declared reset or
proved phase separation can reduce the bound from a sum to a maximum. Linear
growth-bound inference is specified in
[`growth_inference_and_allocator.md`](growth_inference_and_allocator.md); do not
build a general AARA/string solver or whole-program arena inference.

## Remaining design decisions

1. How far can `inout`, row inference, and lifetime elision reduce affine-handle
   noise before explicit Arena identity is required?
2. When must an API state a worst-case input refinement rather than returning a
   fallible capacity outcome?
3. Does bounded recursion also require a first-class stack-byte proof, or is
   stack accounting an external artifact?
4. **Resolved:** an Allocation borrows its Arena, reset rejects while any
   Allocation is live, and structural multiplicity propagates contained linear
   debt. Consumption must discharge or move every obligation before
   reclamation; bulk free never substitutes for element consumption.
5. **Resolved:** Cathedral's splittable physical-memory authority is `Extent`,
   a lower layer. An Arena may borrow an Extent as backing but cannot mint or
   replace its range authority.
6. Must generic-machine instantiation land before an ergonomic generic `Vec`,
   or can the first dynamic container ship only at concrete instantiations?
