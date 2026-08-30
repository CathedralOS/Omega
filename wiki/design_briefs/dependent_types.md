# Design Brief: Dependent Types — the Systems Fragment, Lifetimes, and the Lean Path

Companion to [Chapter 12](../language_guide/chapter_12_dependent_types.md) (the
user-facing surface) and
[proof_engine_north_star.md](proof_engine_north_star.md) (the automation/kernel
fork this feature climbs). Sources: a six-track research sweep (theory, systems
prior art, Rust history, Lean internals, runtime lowering, repo substrate);
key citations at the bottom.

## 1. The problem

Three systems-program shapes motivated this design:

1. **The UEFI memory-map walk.** Firmware returns a buffer holding `count`
   descriptors, each `stride` bytes wide — and stride is a RUNTIME value
   larger than the descriptor struct you compiled against. Walking the buffer
   at `i * stride` needs the proof `i*stride + stride <= len` where every
   term is a runtime value. Striding by the compile-time struct size instead
   is the classic firmware bug that corrupts every entry after the first —
   and under this design it does not compile, because no fact ties that
   constant to `len`. Cathedral M2's recast consumes exactly this shape.
2. **Row-major indexing.** `pixels[y*W+x]` with `x < W` and `y < H` is in
   bounds, but the proof needs `y*W + x < W*H` — a relation between runtime
   values, not a constant range.
3. **Signatures whose result bounds are their arguments.**
   `clamp(value, min, max) -> out in min..=max` — today a bound must be a
   constant.

All three reduce to one missing capability: **ranges, facts, and layouts may
only name constants today; they need to name in-scope program values.**
Constant bounds cannot say it; dominating guards can establish some of it
locally but cannot carry it across a signature.

### The surface is already latent in the language

Three existing constructs already depend on values, which is why the chapter
can be declarative rather than inventive:

- **A slice is a length traveling with data.** `items: &[u8]` carries
  `.len`, and indexing obliges `index < items.len` — a fact naming a runtime
  value. The fat descriptor (ch20) is the layout half; the length fact is
  the proof half. Every rule in chapter 12 generalizes what slices already
  do.
- **A case payload's facts hold under the case fact.** Inside a
  `P::One { v } ->` arm, `v` carries `One`'s declared payload range because
  the arm proves which case is active. A fact valid
  conditionally on another fact IS dependency.
- **Const parameters already reach ranges and lengths.** Ch13's
  `FixedBuffer<T, const N: u64>` puts a value in a layout; ch7's clamp
  contract writes `ensures out in min..=max` over `const` parameters, and
  ch7's own prose says bounds may name "compile-time or proof-visible
  values." The feature is deleting the `const` restriction from that
  sentence.

## 2. What dependent type theory actually contains — and what it costs

The full apparatus, with the builder's verdict on each:

| Mechanism | Buys | Costs | Systems need? |
|---|---|---|---|
| Pi (result type computed from argument value) | value-indexed APIs; quantifiers; generics-as-instance | a normalizer *inside* the type checker (conversion checking evaluates open user terms at compile time); undecidable inference; elaboration | **No.** Layouts/facts *parameterized* by values never require types *computed* by code |
| Sigma (dependent pair) | existential returns; length-prefixed wire data — `{len, payload[len]}` IS one | near-zero in a decidable index fragment | **Yes** — the single most systems-relevant object. UEFI GetMemoryMap returns one |
| Indexed families (Vec) | compile-time-impossible cases | index unification, K-axiom, forced-argument erasure | No — sum types + fact-conditioned cases + dominating guards reproduce the effect through sum-payload narrowing |
| Universes | classify values and formulas | Girard's paradox management only if universes themselves become freely first-class | **Yes, internally:** `Type` for objects and `Prop` for formulas; neither is currently a runtime value or an open source-level universe |
| Definitional equality / normalization | silent computation in types | the checker's termination = the termination checker's soundness; Lean's main pain center (defeq debt, kernel blowups) | No — an entailment *engine* deciding equalities in a decidable theory is the third road: reflected-equality ergonomics without undecidable checking |
| Erasure (QTT quantities 0/1/ω, Idris 2) | proofs and specification-only ghosts cost nothing at runtime | Type relevance must compose with multiplicity, effects, and validity scope | **Partly built:** current facts erase and Type multiplicity already propagates; explicit relevance must replace structural proof-only classification |

**The spectrum, and where Omega sits.** Constant-bound refinements →
*symbolic bounds over linear integer arithmetic, decision-procedure-discharged*
(Dependent ML, Xi & Pfenning — the canonical decidable fragment) → refinement
inference (liquid types) → measures over data → full Pi. Omega today is a
store-enforced, flow-sensitive refinement system at the constant-bound point
with the engine (polynomials + difference-bound matrix + intervals + gated
induction) already built for the *symbolic* point. The feature is the step
from constants to atoms — the DML design point, reached by widening the
existing engine's inputs, not by new theory. What Omega already implements
under its own names: guard narrowing = flow-sensitive refinement; wire data
with runtime strides = Sigma types in disguise; store enforcement = the
ownership-sound strong updates Flux (Liquid Types for Rust, PLDI 2023) showed
make refinement of mutable memory work. The current engine erases proof facts;
the terminal-Psi destination additionally retains kernel-checkable proof terms
and certificates under an explicit relevance judgment.

### Proof-static indexed domains do not make universes runtime values

An erased domain may eventually take a canonical first-order static value as
an index. This generalizes `const N: u64` to eligible structured values and
lets a generic result carry an index constraint computed from input indices.
Units, coordinate frames, currencies, tensor shapes, and protocol encodings
are library customers; none becomes compiler vocabulary.

This does not introduce types-as-runtime-values, predicate indices,
runtime-dependent layout, or arbitrary machine evaluation in type equality.
It lives inside the existing internal `Type`/`Prop` classification without
exposing either universe as data. The nominal domain family remains fixed and
the index is a normalized constraint fact.
Compatibility between an actual index expression and an expected one creates
a verification condition. Closed evaluation or canonical normalization decides
identity; established local facts discharge remaining compatibility obligations
without redefining it. Proof-machine `ensures` enter that same local context,
so no indexed-domain-specific citation syntax exists. The compiler performs no
ambient lemma search and never invents a public generic precondition.

Index eligibility is a structural compiler judgment: the value kind must have
decidable equality and one unique canonical form. Records of eligible fields
may qualify; current `Rat` additionally requires an index-site proof that its
denominator is positive, its signed coordinates are cancelled, and its
numerator magnitude and denominator are gcd-reduced. Runtime floats, pointers,
references, capabilities, and allocation identities do not. A conformance
cannot assert
eligibility. Open symbolic expressions normalize only under an exact selected
algebraic conformance whose laws were checked, not admitted.

Current compiler coverage includes:

- structural eligibility and canonical identity for closed eligible integers,
  booleans, fixed arrays, records, cases, and normalized structural `Rat` values;
- erased domain families with explicit carrier/const telescopes, canonical
  binding constraints, specialization in both engines, and no carrier-layout
  contribution; and
- computed open result indices that retain the selected operation and proved
  algebra authority. Closed normalization or exact active `requires`/`ensures`
  evidence discharges compatibility; unresolved equality rejects without
  ambient lemma search or an index-specific citation surface.

### The proof-side proposition-family fragment

The systems verdict above remains unchanged for runtime data. The
law-bearing-quotient ruling adds one ordered proof-side fragment that did not
exist when the original ladder was sequenced:

```omega
proposition R<I, J>(left: C<I>, right: C<J>);
```

`C` is a proof carrier family with a typed proof-static index telescope, and
`I`/`J` are independently quantified index packs. Evidence is a retained
carrierless erased term produced by a privately selected conformance and
projected through the proposition's declared interface. This is
proposition-valued dependency only:
it does not admit arbitrary value-to-runtime-`Type` computation, runtime proof
fields, value-directed layout, or general Pi-type normalization.

Independent packs are local to the proposition declaration. Carrier parameters
have no global relation roles: another proposition over the same `C` may use
one shared pack and require exact indices. Constructor lifting consumes a
proposition-valued heterogeneous `Lift<I,J,R>` member selected for the exact
quotient/container pair. Dependent fields are processed in dependency order;
a coarser relation on an earlier field generates an explicit transport
obligation for every later proposition application that no longer coincides.

This fragment is a hard predecessor of evidence-bearing quotients. The
relation-property hierarchy (`Reflexive`, `Symmetric`, `Transitive`,
and `Equivalence`), `%`, and the proposition applications used by selected
ordinary lifting theorems cannot be implemented independently from it. The
complete formation and lifting rules live in
[Law-Bearing Relations, Evidence, And Quotients](law_bearing_relations_and_quotients.md).

## 3. (A) The limited systems fragment

Judged against the drivers, the shippable fragment is six pieces — and the
survey is unanimous that nothing more ships anywhere in systems code:

1. **Erased symbolic facts, stored witnesses, nothing else.** Facts about
   runtime values live only in the engine; every witness is a real field or
   parameter the program already carries. No ghost values, no proof terms
   (ATS's fatal UX: manual proof-tuple threading), no quantities (borrows
   already own linearity).
2. **Relational refinements: ranges/domains/contracts may name in-scope
   atoms.** `i: u64 [0..items.len]`, `requires a.cols == b.rows`. The DBM
   already represents value-vs-value order facts natively; this is surface +
   atom plumbing, not engine theory. Guard establishment extends from
   value-vs-constant to value-vs-value (the matrix-agreement driver costs
   *only* this).
3. **Value-parameterized views and wire layouts.** Stored parameters (a field
   sizes a sibling region — the length-prefixed packet; SPARK discriminated
   records since 1983; Linux's `__counted_by` converging on it from below)
   and view parameters (a borrowed byte view carries count/stride fixed at
   establishment — the memory map). Deliberately NOT unified with const generics;
   Rust's const-fragment agony is the counter-example.
4. **Mutation discipline enforced by borrows.** A witness is frozen while any
   dependent borrow lives (loan on the witness); a witness write the checker
   cannot prove coupling-preserving opens an invariant window closed at the
   next consumption point (ch11, settled 2026-07-17 — see §6a). Ada needed a
   whole-record assignment decree and forty years of aliasing-rule patches
   (Ada 2005 ultimately *banned* pointers to mutable-discriminant objects); a
   borrow checker plus consumption-point windows gets the same theorem from
   two structural rules.
5. **Two bridges only: guard establishment and `as` qualification.** Proven, or explicitly
   established at a visible guard/decode, or rejected. No implicit runtime
   checks (§6 decides this fork structurally).
6. **One nonlinear rule: bounded-product monotonicity** over the canonical
   polynomial engine (`0 <= a <= A ∧ 0 <= b ⟹ a*b <= A*b`). This single
   closed rule crosses the exact line where DML/ATS automation historically
   stopped and covers both nonlinear shapes systems code actually produces:
   `i*stride` and `y*W+x`. No solver. (The Mariposa study measured 2.6–5%
   query instability in production SMT-backed verification, with solver
   *upgrades* making it worse — the quantitative case for the owned
   deterministic engine.)

**Exclusions, with evidence:** full Pi (no driver needs a type computed from
a value; ATS's power beyond the index fragment is what nobody could afford to
use), quantifiers (all drivers reduce to quantifier-free per-access
obligations; element facts on types replace `forall` specs), general SMT
(Mariposa), proof terms (ATS), quantities (Idris 2 solved a problem borrows
solve). Refinement *inference* beyond flow-sensitivity is staged, not
excluded: Liquid Haskell's evidence is a ~30× annotation reduction
(DML needed ~31% of program text as annotations; liquid inference cut it to
~1%), but guard narrowing already delivers the intra-procedural share.

**Delta from today (engineering inventory):** symbolic range endpoints +
value-vs-value guard establishments; the which-case and slice-length fact kinds
(decision 18's initial list — already frozen); loan-on-witness; couplings ride the
default-domain build-out (settled, unbuilt); the product rule; frames (§5);
out-params-as-witnesses for boundary ensures (no existential machinery —
mutation through `&mut` already names the witness).

## 4. (B) Lifetimes — why Rust stalled, and what changes here

Rust's blockers are documented, not folklore:

- **RFC 2000 (2017) punted equality in writing**: abstract const expressions
  unify only when "literally references to the same AST node"; const
  well-formedness was left unresolved and still underlies today's
  post-monomorphization errors (`generic_const_exprs` unstable 5+ years).
- **min_const_generics shipped only int/char/bool** because type equality
  must stay deterministic/reflexive — `NaN != NaN` would make a type unequal
  to itself. Valtrees exist because type-level constants compared by
  allocation identity ("two references to equal data compare unequal").
- **The lifetime wall is a pipeline fact**: E0771 / PR #74051 banned
  non-`'static` lifetimes in const generics; const-eval and monomorphization
  run on *erased* regions ("for codegen, all erased regions are treated as
  equal"), so values-in-types would be compared at a phase where lifetimes no
  longer exist.
- **Types are properties of declarations, not program points.** With no
  flow-sensitive facts, a value-dependent type behind `&mut` freezes its
  indices forever (the `mem::Discriminant<Enum<'a>>` invariance cascade,
  issue #74052); and implied-bounds-plus-variance already lets safe code
  forge `&'static` (issue #25860 — open since 2015). Formalizing even
  lifetime-only dependency took a decade (Oxide; Polonius unshipped after
  8+ years).

**The split.** Rust-*specific* (sidestepped by a facts architecture):
type-identity unification, coherence, post-monomorphization errors (Omega's
instances are always spelled; well-formedness discharges at the spelling
site), the erasure-ordering wall (facts and loans check in the same phase),
invariance cascades (subsumption is entailment, not type equality).
*Fundamental* (remain in any design): the decidable entailment fragment IS
the dependent-type fragment (nonlinear arithmetic sits on the cliff — hence
exactly one product rule); canonical equality for fact-position values
(integers only; floats/pointers would reimport valtrees); termination of
fact-level evaluation (the NO RECURSION directive is load-bearing here);
premises must stay riveted to binders; contracts are the only cross-call
transport.

**The Omega answer, in one sentence:** dependencies are flow-sensitive
*facts* about places, never part of type *identity* — so type equality stays
nominal and decidable, mutation kills facts instead of corrupting types, a
borrow of a dependent place read-loans its witnesses, and the strong-update
primitive is the invariant window — write freely, re-prove at consumption
points (exactly Flux's ownership-backed strong updates, which ship with no
relax-like construct at all).

**The new bill:** the borrow checker is promoted to soundness oracle for the
entire proof layer — an aliasing bug now falsifies proven facts, not just
memory safety. (Follow-up worth pinning: differential *fact* checking in the
interpreter oracle — trap when a statically proven fact is false at runtime.)
And facts need frames (§5) — that is where this architecture pays what Rust
pays in unification.

## 5. Frames — fact preservation across effectful calls

Ownership supplies the baseline frame: a call can mutate only places reachable
through exclusive borrows and separately authorized capability state. Checked
bodies refine that baseline with exact inferred mutation summaries. Opaque
calls retain the conservative signature-derived bound.

Omega's floor is uniquely high: **declared ranges, domain memberships, and
default-domain couplings survive every call unconditionally**, because calls
and returns are consumption points: a callee cannot return, call onward, or
hand out a borrow with an open window. A `len` sizing its `payload` crosses an effectful call with
zero annotations — no surveyed language gets this at this price. Only
flow-scoped extras (guard narrowings, established subdomains) die, atom-wise, on
written places.

The rule: *a call may change exactly what its signature admits — the paths
reachable through exclusive borrows plus the abstract regions of its declared
capability reach. Checked bodies refine that set with inferred implementation
summaries. Opaque calls and unknown dynamic conformances remain maximal over
their reachable mutable places. Caller havocs atom-wise on that set; ordinary
`ensures` restores any preservation guarantee the public interface chooses to
publish.* Narrow mutable signatures state useful structural precision directly.
States: the signature is the arrival contract — parameter refinements +
state-level `requires`, proven at every in-edge, assumed at entry, consumed
as the induction hypothesis by the existing strict-decrease rung. Cyclic
states need no loop-invariant construct; Houdini-style guess-and-check over
the deterministic engine auto-recovers the counter/bounds class (0–2
hand-written facts per cyclic state, the Dafny/Why3 norm). Boundary and
capability-carrying calls are consumption points even when they do not name
the windowed place (a capability is a licensed path to state outside the
signature — "cannot observe the window" is unprovable for it), so every
window closes before the world can look.

The compiler retains explicit state `requires` through checking and
specialization, discharges them on every named incoming edge, and assumes them
only after arrival. Statement invalidation prevents a back-edge assumption from
proving itself after mutation. Inferred frames use one sorted complete-or-opaque
representation, compose across resolved internal calls, nested boundary calls,
and acyclic state graphs, and substitute state parameters positionally. When a
body cannot be summarized more narrowly, the caller retains the ownership
ceiling formed by the receiver and every place-shaped argument that could be
exclusive; a place that cannot be represented still makes the frame opaque.
Body-derived frames remain
implementation evidence outside public contract/specialization identity.
Write-only parameters use the same place-origin and frame composition, but the
callee may neither load their referents nor forward them to a readable
parameter. Exact outcome frames preserve facts over unchanged paths and
invalidate facts only where a write occurred; legality and post-write validity
may depend on static structure, written values, and deliberately supplied proof
facts, never on observation through the write-only loan.
Cycles may freely reorder primitive values and shared references because those
parameters cannot redirect caller-visible writes. A named state SCC also
retains an exact finite frame when every cyclic edge forwards all write-capable
parameters through a bijective permutation: the checker solves the positional
may-write equations to a fixed point and instantiates the result through
resolved callers. Within one state, a direct `self` loop, or an acyclic named
state graph, a stable local mutable alias initialized from an exact `self` or
state-parameter place substitutes that origin through positional transition
arguments, including member suffixes and resolved calls. A stable bare local
reborrow (`let child: &mut T = &mut parent`) or exact member reborrow
(`&mut parent.field`) flattens through an already-known parent alias. An indexed
origin such as `&mut self.cells[i]` or `&mut parent.group.cells[i]` is known only
at collection precision: an exact parent retains the nearest intermediate
collection (`parent.group.cells`), while an already-coarse parent absorbs every
later suffix. A direct origin such as `&mut self.cells[i].value` likewise
publishes `self.cells`; the member after the index cannot narrow the collection.
That coarsening remains absorbing through later alias projections, member
writes, calls, and transitions. A call result inherits its argument's exact or
collection-coarse origin only when a free helper's entire body is one terminal
place rooted in one mutable-reference parameter. Exact member suffixes compose;
an indexed result coarsens at its nearest collection and stays absorbing through
later transparent calls and reborrows. Lifetime elision by itself is
insufficient. Potential rebinding, local or computed collection origins,
computed or nontrivial call results, and transport through a named state SCC
remain opaque, except that primitive scalar and recursively primitive
fixed-array locals are caller-isolated: writes through their exact or
collection-coarse aliases disappear from the caller-visible frame. Named,
reference-bearing, and generic local roots do not receive that exception. A
direct-call tree through depth two may initialize a caller-isolated scratch
local without obscuring a separately returned parameter origin only when every
inferred frame is complete and every write resolves into a previously
established caller-isolated scratch local; an empty frame is the degenerate
case. Deeper, recursive, computed, opaque, or externally writing initializer
calls remain fences. One direct Unit statement call with a complete frame may
likewise precede the terminal place when its arguments do not expose a mutable-
reference binding for rebinding; writes through references passed by value change contents
without redirecting their origins. Sibling direct value-call arguments are
independently admitted when their expressions are non-rebinding and every call
frame is complete, including nested direct calls to a maximum call-tree depth of
two. Explicitly discarded call results, explicit binding reborrows, deeper
computed arguments, and any opaque node remain fences.
An internal statement call may take a mutable indexed argument whose index is
the same complete non-rebinding direct-call tree through depth two. Caller-
alias-aware frame instantiation coarsens the callee's argument write to the
collection, while ordinary evaluation publishes every index-call write.
The argument may index through a stable helper-local mutable alias; its
established origin supplies the collection. It may instead index a structurally
transparent helper result directly, with the helper's returned-place relation
supplying the collection. The compiler-owned `as_mut_slice()` view is neutral
on that argument spine, including after a stable helper-local mutable alias, a
transparent free helper result, or an attached helper result rooted in its
actual `self` receiver: the callee write rebases through the alias or helper and
view to its backing array before the index coarsens it. Deeper index trees and
recursive or opaque free/attached view
producers remain fences. An exact member projection may be carried by the
stable alias or follow a free or attached helper result before the view; the
suffix composes before view preservation and index coarsening, while a member
after the index remains absorbed by the coarse backing collection. With
repeated indexes, the first fixes that coarse collection, later indexes stay
absorbed, and every independently bounded index frame publishes. An attached
helper may likewise root that relation in its actual `self` receiver. An exact
member projection may follow the helper
result before one or more indexes: the suffix composes first, the first index
coarsens to that nearest collection, and
later indexes or members are absorbed; each index expression independently
satisfies the same bounded-call rule.
Recursive or opaque free/attached collection
producers, boundary calls, and deeper or binding-reborrow index trees remain
fences.
A value-shaped assignment also preserves a separately returned parameter origin
when its right-hand side is a typed non-reference direct-call tree of maximum
depth four and every frame is complete. Sibling branches are admitted
independently, and every nested-call write remains published. One deeper,
binding-reborrow, recursive, or opaque branch fences the whole right-hand side;
reference-valued roots retain their existing relational handling. A direct
primitive scalar value may wrap complete caller-isolated call producers in up
to nine unary, binary, primitive-cast, member-projection, or indexing shells
under that same call budget. A tenth shell or a generic, reference-valued, or
unknown call result remains a fence. Aggregate fields and projected concrete
record, selected-case, and fixed-array literals retain their separate two-shell
computation budget. One
top-level concrete primitive-only record or selected-case literal may likewise
contain an independently bounded call tree in each direct common or payload
field. Direct typed assignment values may nest concrete primitive-only record,
selected-case, and literal fixed-array aggregates through depth three; every
primitive leaf obeys the same rule. At any admitted aggregate level, a declared
primitive field may wrap those call leaves in up to two scalar computation
shells formed from unary or binary operators, primitive value casts, member
projections, or indexing. The
direct aggregate-depth-three and computed-depth-two rails do not widen the
depth-four call budget. A fourth direct aggregate level, generic, recursive, or
reference-bearing carrier, and other computed field shapes remain fences.
A primitive assignment may directly project a field from one such concrete
record/case literal or index a fixed-array literal. The projection consumes one
of the two computation shells. Every eagerly evaluated literal field or array
element publishes its bounded call writes; the remaining shell may be used by
an element computation or by one outer scalar computation, but not both.
Projected record/case and directly indexed array literals retain their separate
aggregate-depth-two rail; a third projected aggregate remains fenced.
A value-shaped assignment through an indexed target likewise preserves a
separately returned parameter origin when the collection projects through a
stable helper-local mutable alias or an exact transparent helper result and
each of its one or more indexes is a complete, non-rebinding direct-call tree
of maximum depth two. The first index fixes the collection-coarse target, later
indexes are absorbing, and the ordinary frame publishes every index-call
write. The compiler-owned `as_mut_slice()` view may occur on the collection
spine, including after a stable helper-local alias, a transparent free helper
result, or an attached helper result rooted in its actual `self` receiver: it
preserves that source's backing array origin before the first of one or more
indexes coarsens it; later indexes stay absorbed and each bounded index frame
publishes. Recursive or opaque free/attached view producers remain fences. An
exact member projection carried by a stable alias or produced by a helper may
precede the view: its suffix composes before the view preserves that exact
origin for later indexing; any member after that index remains absorbed by the
coarse backing collection. A transparent free helper result or an attached
helper result rooted in its actual `self` receiver likewise supplies the
collection origin without an intermediate binding. An exact member projection
may follow that result before
one or more indexes: the suffix composes first,
the first index coarsens to that nearest collection, and later indexes or
members remain absorbed while every independently bounded index frame
publishes. Deeper or
binding-reborrow index trees and recursive or opaque free/attached collection
producers remain fences.
The bounded indexed target and bounded non-reference value tree may occur on
the same assignment. Their complete frames compose and publish independently;
either side exceeding its depth or non-rebinding rail fences the returned-place
relation. A compiler-owned mutable-slice view on the target collection is
neutral to that composition: the target index and value tree retain independent
depth-two and depth-four budgets, respectively, and publish all call writes.
Terminal returned places, stable local mutable aliases, and direct alias rebind
replacements may contain one or more indexes whose non-rebinding call trees are
independently complete through depth two. The first index fixes the coarse
collection origin; later indexes are absorbing, every index frame publishes, and
only the rebound name moves while prior reborrows retain their origins. A
compiler-owned `as_mut_slice()` view before the first index preserves the
returned, initializer, or replacement collection's backing origin. Deeper,
binding-reborrow, recursive, or opaque index forms remain fences.
Non-bijective, computed, or otherwise
unrepresentable cyclic rebinding retains only the coarse ownership ceiling;
`TASKS.md` R5 owns further relational candidates.

## 6. Dynamic lowering — the runtime half

Ada is the direct ancestor: discriminated records have shipped value-dependent
layouts since 1983, with size/offset arithmetic compiled from stored
discriminant fields, and the constrained/unconstrained object split mapping
*exactly* onto static/dynamic lowering of one type. SPARK/GNATprove is the
synthesis precedent: every runtime check becomes a verification condition;
Silver level = all checks proven; only then is compiling checks out sound
(suppressing unproven checks is formal UB — a soundness cliff Omega's model
does not have). Swift's value witness tables are production dynamic-layout
machinery solving the *wrong* problem (ABI resilience via hidden global
metadata); the transferable lesson is only size-vs-stride and the
alloca/boxing swamp that runtime-sized *locals* drag in — hence the rule:
dynamic-sized regions live behind views or fixed-capacity buffers; owned
runtime-sized values wait for Arena-backed Allocation.

**The no-implicit-checks fork is decided structurally, not aesthetically.**
Implicit residual checks (hybrid typing's "maybe" casts) require an ambient
failure continuation — exceptions — which Omega deliberately lacks; blame
theory exists solely to debug system-inserted checks failing far from their
faults, an apparatus deleted by having no inserted checks. Eiffel is the
cultural warning (configurable monitoring decays contracts into
documentation); Ada avoids decay only via the UB cliff. Omega's guards with
forced false arms + proved `as` qualifications are the entire runtime story, composed from
existing parts. The honest costs, stated: engine incompleteness becomes
user-visible guard friction (mitigate with an obligation catalog — errors
name the missing fact and the minimal guard), and obligations must float
into `requires` so libraries export them instead of guarding internally —
without contract floating the stance is compositionally unusable.

### ZII and gating

Gating resolves the ZII/invariant tension:

- Every type has a default domain, declared with the data declaration
  (undeclared = the empty domain). ZII stays a **storage** guarantee
  unconditionally: the all-zero pattern is always valid bits, memset always
  legal, never UB — it constrains the compiler, never the programmer
  (ch20's existing words).
- If zero satisfies the default domain, the type is zero-constructible: a
  zeroed value is born established, the facts are standing everywhere,
  nothing is tracked. This is the zero-constructible tier.
- If zero does not, the type is **gated**: not zero-constructible. Data can
  have non-zero requirements — business logic cannot thrive under a
  zero-init-everything law. The zeroed form exists only as storage and is
  inaccessible as the type until construction or an `as` qualification proves the
  domain. Establishment is monotone as observed (a later write may open an
  invariant window, but every consumption point closes it, so no observer
  sees a place fall back to hidden) — a cheap one-way fact riding the
  arrival-facts machinery, not typestate bookkeeping.
- Construction is the gate: a gated type's literal must prove the domain, so
  exactly the fields whose zero violates it are mandatory
  (`Player { health = 50 }` — the ch7 settle's construction semantics,
  derived rather than separately decreed).
- The gate propagates through containment (a container of gated data is
  gated) and is **absorbed by a zero-valid first sum case** — emptiness is
  spelled as a case (`PlayerSlot::Empty`), not as a nonsense zero value.
  Machine-owned data is access-gated, not construction-gated (Main boots
  zeroed; gated fields are storage until a state establishes them).
- The current 0-in-range declaration check becomes the first tier as an
  implementation restriction, not language law.
- Deferred honestly in every universe: partially-established arrays under
  runtime indices ("elements below `loaded` are established" is a
  quantified fact — the quantifier rung).

Decode establishment grants exactly the checked predicates and nothing more —
firmware semantic truth is not decodable. Length witnesses at zero mean
empty (len=0 → nothing to access) for zero-constructible types; gated types
are never observed zeroed.

### Invariant windows

Invariant preservation uses **consumption-point enforcement**. Writes never
fail a domain check. A write the checker can
prove domain-preserving changes nothing (facts stay standing for ordinary
proved range-checked stores). A write it cannot prove opens a
**window** on the place; the window must close — the domain re-proven from
flow facts — at the next **consumption point**: a read relying on a domain
fact, a borrow creation, any call, a transition, return/scope expiration,
or any boundary/capability-carrying call (the world observes memory).
Errors cite both ends (opening write, failing consumption point).

Why it is sound with nothing new: exclusivity — relax's soundness hinge and
its unbuilt enforcement pass — is the borrow checker (no aliased mutation;
a live dependent borrow pins witnesses, so windows cannot open under
observation). Why the dependent-types results survive: the frames floor
("couplings survive every effectful call, zero annotations") restates from
*every store proves* to *every escape proves* — calls and returns are
consumption points, so a callee cannot leak an open window; gating's
monotone establishment becomes monotone-as-observed. Prior art: Flux
(PLDI 2023) ships ownership-backed strong updates with no relax-like
construct — refinements demanded at weakening/call/return points — which is
this exact model.

What `relax` was, in hindsight: manual window management, designed before
absorbing that the flow-fact catalog plus borrow exclusivity infers the
windows. Its rules survive as theorems (no-transitions-inside = transitions
are consumption points; restore-at-exit = return is a consumption point;
callee-must-take-relaxed-view = calls are consumption points, split helpers
over decoupled fields). Residues, honestly: mid-window whole-value helper
signatures (`&mut relaxed T`) are retired in favor of field-decoupled
helpers, revisited only if a real case demands the whole; multi-state
phased construction stays impossible (same wall, simpler statement);
runtime-indexed array writes window the whole array under unknown indices
(identical conservatism to whole-array relax — no regression). Use-site
error distance is mitigated by citing the opening write. ch11 is rewritten
as the Invariant Windows chapter; ch7/ch8/ch9/ch12 and the appendix are
restated.

## 7. (C) The Lean-competitive expansion

Lean's trusted base is a small kernel (CIC + inductives + quotients +
definitional proof irrelevance + universe polymorphism); everything else —
elaborator, unification, typeclasses, tactics — is huge *and untrusted*, made
operational by an export format and external re-checkers (lean4lean re-checks
all of mathlib and found a real kernel bug; Lean 3's checkers died when the
export path rotted — the format IS the guarantee). Lean's own pain points are
instructive negatives: its ideal definitional equality is undecidable
(Carneiro 2019 — the implemented check is a non-transitive
under-approximation), and defeq performance debt forced well-founded
definitions irreducible-by-default in 4.9. A later language should keep
judgmental equality trivial and make all interesting equality
derivation-backed — which is the engine architecture Omega already has.

The staged path (no rewrite at any stage):

1. **Quantifier fragment, natively framed** — element facts over views,
   domain invariants, machine invariants; witness-carrying existentials only
   (out-params already skolemize); deterministic instantiation at fixed
   program points under an explicit budget. Kills F*-style trigger
   instability by construction; covers SPARK/Dafny-class properties and most
   of Cathedral's needs.
2. **Derivation records.** Every engine component already computes an
   implicit derivation (DBM triangle steps, polynomial rewrite chains,
   interval monotonicity, Farkas coefficients, induction triples); emit them
   in one shared record format. The composition is proven in four ecosystems
   (Sledgehammer/Alethe, SMTCoq, lean-auto/Duper — 98% of cvc5 certificates
   replay in <1s, bv_decide's untrusted-solver/verified-checker split).
3. **A few-kLoC checker + documented export format.** Trust inverts: the
   engine becomes the untrusted front line, the checker the kernel — zero
   change to engine algorithms. F* is the cautionary neighbor (no kernel, no
   export, TCB = typechecker + Z3, and no mathematics library at scale;
   automation strength was never its gap).
4. **The ghost stratum.** Recursive ghost data is gated on the existing
   strict-decrease measure (never definitional unfolding — Lean's lesson).
   Before quotient formation, add the `proposition`-family/index-telescope
   fragment and carrierless selected-conformance evidence specified in the
   law-bearing-relation brief. Relation properties are explicit composable
   conformances; quotient lifting explicitly selects an ordinary checked
   result-congruence theorem, while the quotient-facing author publishes `Q`
   and proves legality for both representative calls. Exactly two
   universe levels remain the starting ceiling until category theory forces
   more. This opens algebra, combinatorics, number theory, and construction of
   the reals. QTT/Idris 2 is the proof that one calculus hosts erased
   mathematics beside linear runtime resources; Omega's split is cheaper still
   because ghost values are never borrowed and need no layout.
5. **Proof machines as the escape hatch** — state machines run at compile
   time by the existing interpreter, terminating by the existing ranked-cycle
   discipline, emitting derivation records the checker validates. No foreign
   "tactic" concept; mathlib hammer data (~37% automatable) calibrates the
   front-line/escape-hatch split.

**The foreclosure ledger** — near-zero-cost mitigations to adopt NOW so the
systems fragment never blocks the math rung:

- Never define fact-truth as "the engine accepts it" (the F*/Dafny trap):
  write every engine rule as a declarative inference rule in the design
  record; "engine accepts" stays a documented under-approximation of
  "derivable", and that rulebook becomes the Stage-3 kernel spec.
- Scope qualification `as` to runtime carrier domains; a
  proposition is not a decodable thing.
- ZII and layout obligations never apply to propositions (a zero-inhabited
  proposition is inconsistency by construction); never write "every value
  has a machine layout" into the record — ghost unbounded integers arrive in
  Stage 4.
- Scope NO RECURSION to the runtime stratum — induction IS ranked recursion,
  and the `terminates by` discipline is the gate.
- Never make folder-normal-form identity the semantics of fact equality (the
  existing const-fold bug class would turn from completeness gaps into
  soundness holes).
- Keep an extension node in the fact AST so engine limits stay diagnostic
  ("cannot prove"), never grammatical ("cannot say").

Explicitly safe existing choices (verified against the gap analysis):
store-enforced ranges, always-spelled monomorphized instances (Lean's kernel
also sees only fully elaborated terms), state machines as sole control flow
(a better induction substrate than loops), quantifiers-as-parse-error (given
the AST extension node), and the borrow system (ghost values are copied
freely and never borrowed).

## 8. Implementation boundary

`TASKS.md` owns the live R1-R6 queue. This brief fixes the dependency order:
symbolic atoms and couplings support bounded products and boundary witnesses;
exact inferred frames preserve those facts across calls; typed proposition and
index telescopes then enable carrierless evidence, structural relation lifts,
and explicitly selected ordinary quotient theorems. R6 remains a hard
predecessor of quotient migration. Landed
coverage and individual inference cases belong in tests and Git history rather
than an accumulating status diary here.

## 9. Key sources

Xi & Pfenning, *Dependent Types in Practical Programming* (DML); Rondon &
Jhala, liquid types; Flux (PLDI 2023); McBride *I Got Plenty o' Nuttin'* /
Atkey QTT (LICS 2018); Idris 2; Ada RM 3.7 discriminants + SPARK/GNATprove
levels; HACL*/EverParse (F*/Low*); Mariposa SMT-stability study; Rust RFC
2000, min_const_generics stabilization report, E0771/PR #74051, issues
#74052/#25860, valtrees (oli-obk), lcnr on generic_const_exprs; Oxide;
Polonius; Lean 4 kernel + lean4lean; Carneiro 2019 (defeq undecidability);
Selsam tabled typeclass resolution; Sledgehammer/Alethe, SMTCoq, lean-auto/
Duper, bv_decide; Flanagan hybrid type checking (POPL 2006); Lehmann &
Tanter gradual refinements (POPL 2017); Swift value witness tables; C
`__counted_by` (Linux 6.5+); CIVL yield invariants; Houdini. Full per-track
source lists live in the research transcripts.
