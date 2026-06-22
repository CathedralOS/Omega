# Allocator Design for Omega — Primer

> Landscape + rationale to prime the allocator decision. The concrete staged
> plan + decisions (A1–A5) live in [`allocator_story.md`](allocator_story.md);
> this is the "why / what does everyone else do / what fits us" companion.
> Synthesized from a 6-facet research sweep (Rust, Zig/Jai/Odin, safety-critical,
> regions/multi-heap, proofs/guarantees, Omega-fit); every Omega claim verified
> against `region.omg` / `vec.omg` / `fixed_vec.omg` / `omega-runtime-abi`.

**Where Omega is today.** No heap, no allocator. Storage is inline (`[T;N]`,
struct fields), bounded (`FixedVec<T,N>` — `push` is a *compile-time* proof
obligation `len < N`, never a runtime trap), or borrowed (`{ptr,len}` slice
descriptors). ZII (all-zero is a valid inhabitant) is the soundness backbone.
The allocator is **designed but unbuilt**: `allocator_story.md` (A1–A5, REC:yes,
awaiting sign-off) specifies `Region<'r>` as a capability bound through the
reserved `Allocation` boundary-provider category, with `alloc`/`dealloc` as
effect names. Concretely blocked: `Vec<u8> in Utf8` (owned/growable text),
copy-out wire decode, `read_line(&mut String)`.

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

## The questions you raised, answered
- **"Is multi-heap the future?"** For an OS, largely yes — *but* "multi-heap"
  bundles three independent wins: lifetime (per-request/per-frame bulk reset),
  contention/locality (per-core/NUMA heaps — jemalloc/tcmalloc/mimalloc, an
  implementation detail), and isolation/accounting (per-component quotas — seL4,
  Zircon VMOs). Treat them separately; only lifetime and isolation belong in the
  type system. Per-core/NUMA is a backend detail that should NOT.
- **Avionics/robotics perspective.** The whole field already agrees: **no
  dynamic heap after init.** Three reasons each fatal to certification —
  malloc/free have unbounded WCET (breaks scheduling), general heaps fragment (a
  request fails after long uptime despite free bytes), and OOM is unprovable.
  Enforced today by *coding-standard bans* (MISRA 21.3, NASA Power-of-10 R3,
  ARINC-653 init-only), **not by any language.**
- **"Pre-allocate in main and PROVE it fits / no OOM in flight?"** This *is*
  hard-real-time practice — static budgets, fixed pools, per-cycle arenas. But
  the "proof" is a patchwork: linker budgets + WCET tools (aiT/RapiTime) +
  stack-depth analysis + partial formal proof (SPARK/GNATprove). **No language
  unifies it.** Tellingly, SPARK proves absence of *all* run-time errors
  **except `Storage_Error` (memory exhaustion)** — its answer there is "don't
  allocate dynamically." **That residual is exactly Omega's opening.**
- **"Or solved by not allowing an allocator at all?"** Yes — the no-heap model
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
it reuses capabilities/effects (ch18, shipped), the interval/refinement prover,
lifetimes (decision 15), and the `{ptr,len}` descriptor ABI.

- **A — Stay heap-free (the floor).** `FixedVec<T,N>`/`[T;N]` ship today; OOM
  impossible by construction. *Gap to close first:* generic-machine
  instantiation (FixedVec's real bodies are pinned concrete to i32/N=4; generic
  `data` whose layout depends on `T` fails layout).
- **B — `Region<'r>` arena with a *proven capacity refinement* (the
  differentiator).** `reserve(n)` yields a region whose type carries
  `remaining`; `alloc(r,n)` carries obligation `n <= remaining`, postcondition
  `remaining' = remaining - n`; the region handle is threaded **affinely** so
  the budget can't be double-spent. `alloc` is **infallible after proof**
  (returns a bare handle, no Result) — SPARK's `Storage_Error` residual turned
  into a discharged theorem. *Cost:* data-dependent sizes need worst-case
  *input* refinements (`input: &[u8] [len <= N]`); unboundable sites degrade
  visibly.
- **C — `Vec<'r,T>` borrowing a Region, capacity fixed at `with_capacity`, NO
  growth (allocator-story stage 2; smallest viable unblock).** Bind one
  `Allocation` provider (host malloc), extend the descriptor to `{ptr,len,cap}`
  under its existing owner `omega-runtime-abi`, lower `core/vec.omg` ops through
  `Allocation` + the working `vec_views` provider, wire drops. Unblocks owned
  text/decode/`read_line` with no realloc — reuses FixedVec's
  proof-obligated-capacity model with a runtime capacity measure.
- **D — Demote unprovable sites to fallible `try_alloc -> Result` or the abort
  effect.** Keeps unboundable inputs *visible*, not unsound (mirrors SPARK's
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
model B (`Region<'r>` whose *type* carries a proven `remaining` interval) as the
differentiator no mainstream stack offers. Concretely, **build allocator-story
stage 2 first** (Region provider + fixed-capacity `Vec<'r,T>`, no growth): least
machinery, unblocks the real corpus. Frame allocation as an **effect** from day
one so pure code is *provably* allocation-free. Demote genuinely-unbounded sites
to fallible/abort (D) rather than weakening the proven core. Defer growth,
pluggable allocators, and `try_push` to stage 3 until demand is real. Optionally
layer RAML/AARA-style automatic size inference later to cut annotation burden.

## The rung lattice, the bootstrap, and downstream opt-in
The rungs form a **partial order (lattice), not a total ladder** — memory
(static → stack → fixed-arena → proven-Region → bounded-heap → unrestricted) and
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
source to rungs 0–2 (heap-free / fixed-arena, no prover needed) — rung 3 (proven
Region) is un-admittable until the interval engine is self-hostable. Caveats from
precedent: (a) this binds the seed's *source*, not the running binary at every
stage — real bootstraps (Guix hex0→Mes→tcc→gcc, GCC 3-stage, mrustc) route
*through* a mature host language and allocate freely from the first C-subset rung;
(b) Thompson-resistance is **orthogonal** — Diverse Double-Compilation closes
trusting-trust, the minimal seed buys trust-base shrinkage + per-rung
auditability, and the proof-kernel is a *third* independent ladder. Keep all
three claims distinct.

## Affine-handle ergonomics — recommended design (decision #2)
The affine `Region<'r>` handle is *positional in the move-graph*, so naïvely it's
worse than Zig's allocator coloring (signature tax + hand-threaded version chain +
higher-order virality). Omega is the rare language with all three levers to
collapse the noise — layer them:
- **R1 elide `'r`** (Rust/Cyclone) when one region is in scope. Reject
  Tofte-Talpin whole-program inference — its failure mode is a *space leak*,
  poison for a no-OOM proof.
- **R2 ambient-but-tracked capability** (Jai delivery + Scala/Austral tracking):
  `region r { }` summons the budget at the leaf `alloc`, invisible at intermediate
  call sites, budget tracked in the effect.
- **R3 `inout`/consume passing mode** (Val/Hylo) not rebind-and-return — the
  compiler reconstructs consume-and-rebind and **SSA-threads the remaining-bytes
  interval using the SAME merge/narrowing the arithmetic-domain engine already
  does.** Biggest single win.
- **R4 effect-row inference** (Koka) so higher-order combinators stay polymorphic
  over their callee's `alloc` effect (kills virality).
- **R5 `?`-style sugar** at the fallible boundary (FixedVec already has fallible push).

**Irreducible residuals** (must stay explicit — they ARE the theorem's spec):
declare the budget bound; disambiguate when >1 region is live; mark each
**split/fan-out** (a sequential thread can be hidden, a partition cannot); mark
each fallible escape hatch; state `outlives` at escape.

## Open design decisions (what you must pick)
1. **Net vs peak — RESOLVED to peak.** Track **high-water-mark (peak live set)**,
   not net — a persistent arena never frees mid-life. Lever: proving two phases
   **don't co-exist** makes peak `max(A, B+C)` not `A+B+C`; a **region reset
   between phases IS that disjointness proof**. Sub-decision: prove non-coexistence
   via automatic liveness (fragile) or declared region/phase boundaries
   (predictable — rung-2 arena reset gives it free). *Leaning declared.*
2. **Affine-handle ergonomics — briefed above (R1–R5 + residuals).** Open: how far
   `inout` + effect-row inference + elision collapse the noise before the
   residuals bite.
3. **Input-bound policy.** When is a site *required* to carry a worst-case input
   refinement (`input: &[u8] [len <= N]`) vs allowed to demote to fallible/abort?
   Embedded culture caps inputs; general-purpose stays fallible — pick a default.
4. **Stack depth — clarified.** Omega HAS recursion and bounds it via
   `terminates { decreases }` (gives *termination*, canaried incl. a runtime
   recursive value-call). "No recursion" is only a future Cathedral stackless-task
   stance. Bounded recursion ≠ a proven *stack-byte* bound — whether stack depth
   is a first-class discharged obligation (vs external tool) is the open question.
5. **Destructors on bulk region free (A5).** Must arenas *reject* storing
   affine/capability-owning values (or prove cleanup trivial) to avoid the
   bumpalo Drop-skipping footgun, given element drops are separate from bulk free?
6. **`SharedRegion<Untrusted>` — RESOLVED in the IPC deep dive** (Cathedral repo
   `part_3_communication/00_ipc_and_service_invocation.md` + `cathedral_alignment.md`).
   It is **NOT a new memory category**: untrusted bytes arrive as `&[u8, []]`
   (empty proven-invariant set) over existing primitives; the invariant/pointer
   contract system forces snapshot-then-validate *structurally* (a TOCTOU
   re-read/index/tag-read does not typecheck). Remaining (minor): stdlib-only vs
   small compiler help (leaning stdlib); the minimal kernel surface
   (`grant_region`/`set_permissions`/`revoke`/`send_capability`); scheduler layer
   (wake-reason sum, typed `protocol` RPC surface).
7. **seL4 untyped+Retype as the Cathedral *kernel* model.** Should physical
   memory be a splittable capability the prover tracks (parent/child
   derivation), and does that unify with the userspace `Region<'r>` surface or
   sit beneath it?
8. **Generic-machine instantiation.** Is fixing it (FixedVec bodies can't be
   carried generically — layout fails on `[T;N]` with no concrete extent) a
   prerequisite for an ergonomic generic `Vec`, or can stage 2 ship
   concrete-instantiated like FixedVec does today?
9. **Automatic size inference (RAML/AARA).** Worth layering LP-based
   per-function allocation bounds to cut annotation burden, given it's
   polynomial-only and weak on higher-order/closures?
