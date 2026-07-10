# Design Brief: Dependent Types — the Systems Fragment, Lifetimes, and the Lean Path

Drafted 2026-07-15. Status: **direction, not a decided plan.** Companion to
[Chapter 23](../language_guide/chapter_23_dependent_types.md) (the proposed
user-facing surface) and
[proof_engine_north_star.md](proof_engine_north_star.md) (the automation/kernel
fork this feature climbs). Sources: a six-track research sweep (theory, systems
prior art, Rust history, Lean internals, runtime lowering, repo substrate);
key citations at the bottom.

## 1. The problem

Three concrete programs are blocked (chapter 23 opens with them): the UEFI
memory-map walk (`i * stride` with runtime stride — Cathedral M2's recast
consumes exactly this), row-major indexing `pixels[y*W+x]` (TASKS.md: "enabled
by dependent types eventually"), and signatures whose result bounds are their
arguments (`clamp -> out in min..=max`). All three reduce to one missing
capability: **ranges, facts, and layouts may only name constants today; they
need to name in-scope program values.**

## 2. What dependent type theory actually contains — and what it costs

The full apparatus, with the builder's verdict on each:

| Mechanism | Buys | Costs | Systems need? |
|---|---|---|---|
| Pi (result type computed from argument value) | value-indexed APIs; quantifiers; generics-as-instance | a normalizer *inside* the type checker (conversion checking evaluates open user terms at compile time); undecidable inference; elaboration | **No.** Layouts/facts *parameterized* by values never require types *computed* by code |
| Sigma (dependent pair) | existential returns; length-prefixed wire data — `{len, payload[len]}` IS one | near-zero in a decidable index fragment | **Yes** — the single most systems-relevant object. UEFI GetMemoryMap returns one |
| Indexed families (Vec) | compile-time-impossible cases | index unification, K-axiom, forced-argument erasure | No — sum types + fact-conditioned cases + dominating guards reproduce the effect (the landed sum-payload narrowing already is this) |
| Universes | types-as-values | Girard's paradox management; permanent bookkeeping tax | No — a language whose types are never first-class values has **no universe problem**; keep it that way until the math rung |
| Definitional equality / normalization | silent computation in types | the checker's termination = the termination checker's soundness; Lean's main pain center (defeq debt, kernel blowups) | No — an entailment *engine* deciding equalities in a decidable theory is the third road: reflected-equality ergonomics without undecidable checking |
| Erasure (QTT quantities 0/1/ω, Idris 2) | proofs cost nothing at runtime | a quantity system on every binder | **Free already** — facts live in the engine, never as terms; every proof is quantity-0 by construction |

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
make refinement of mutable memory work; engine-not-terms = perfect erasure.

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
   atom plumbing, not engine theory. Guard minting extends from
   value-vs-constant to value-vs-value (the matrix-agreement driver costs
   *only* this).
3. **Value-parameterized views and wire layouts.** Stored parameters (a field
   sizes a sibling region — the length-prefixed packet; SPARK discriminated
   records since 1983; Linux's `__counted_by` converging on it from below)
   and view parameters (a borrowed byte view carries count/stride fixed at
   mint — the memory map). Deliberately NOT unified with const generics;
   Rust's const-fragment agony is the counter-example.
4. **Mutation discipline enforced by borrows.** A witness is frozen while any
   dependent borrow lives (loan on the witness); changing a stored witness
   requires a whole-value rebuild or a relax scope (Ada's whole-record
   assignment discipline, derived here from one structural rule instead of
   forty years of aliasing-rule patches — Ada 2005 ultimately had to *ban*
   pointers to mutable-discriminant objects; a borrow checker gets that
   theorem for free).
5. **Two bridges only: guard mint and `as` mint.** Proven, or explicitly
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
value-vs-value guard mints; the which-case and slice-length fact kinds
(decision 18's v1 list — already frozen); loan-on-witness; couplings ride the
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
borrow of a dependent place read-loans its witnesses, and `relax` is the one
strong-update primitive (Flux proved this shape sound under ownership).

**The new bill:** the borrow checker is promoted to soundness oracle for the
entire proof layer — an aliasing bug now falsifies proven facts, not just
memory safety. (Follow-up worth pinning: differential *fact* checking in the
interpreter oracle — trap when a statically proven fact is false at runtime.)
And facts need frames (§5) — that is where this architecture pays what Rust
pays in unification.

## 5. Frames — fact preservation across effectful calls

The gap the research closed last (no prior coverage in the wiki). Survey
verdict: kill-on-call is the measured worst default (ACSL's
assigns-everything); ownership-based verifiers (Flux, Creusot, Verus) ship
**zero** frame clauses because the borrow signature *is* the frame; SPARK's
"inferred inside the unit, written at boundaries" coincides exactly with
Omega's frozen contract-inference decision.

Omega's floor is uniquely high: **declared ranges, domain memberships, and
default-domain couplings survive every call unconditionally**, because every
store inside any callee must re-prove them and relax must close before
control returns. A `len` sizing its `payload` crosses an effectful call with
zero annotations — no surveyed language gets this at this price. Only
flow-scoped extras (guard narrowings, minted subdomains) die, atom-wise, on
written places.

The v1 rule: *a call may change exactly what its signature admits — the paths
it takes by exclusive borrow (per-field inferred intra-unit; declared via a
`stores` clause at machine boundaries, mandatory on boundary traits — SPARK's
assume-pure import default is a documented trap), plus the abstract regions
of its declared capability effects. Caller havocs atom-wise on that set;
ensures adds deltas; requires-scope binders name entry values (no `old`).*
States: the signature is the arrival contract — parameter refinements +
state-level `requires`, proven at every in-edge, assumed at entry, consumed
as the induction hypothesis by the existing strict-decrease rung. Cyclic
states need no loop-invariant construct; Houdini-style guess-and-check over
the deterministic engine auto-recovers the counter/bounds class (0–2
hand-written facts per cyclic state, the Dafny/Why3 norm). Inside `relax`:
no capability-carrying or boundary calls, ever (a capability is a licensed
path to state outside the signature — "cannot observe the relaxed target" is
unprovable for it).

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
alloca/boxing swamp that runtime-sized *locals* drag in — hence v1's rule:
dynamic-sized regions live behind views or fixed-capacity buffers; owned
runtime-sized values wait for `Region`.

**The no-implicit-checks fork is decided structurally, not aesthetically.**
Implicit residual checks (hybrid typing's "maybe" casts) require an ambient
failure continuation — exceptions — which Omega deliberately lacks; blame
theory exists solely to debug system-inserted checks failing far from their
faults, an apparatus deleted by having no inserted checks. Eiffel is the
cultural warning (configurable monitoring decays contracts into
documentation); Ada avoids decay only via the UB cliff. Omega's guards with
forced false arms + `as` mints are the entire runtime story, composed from
existing parts. The honest costs, stated: engine incompleteness becomes
user-visible guard friction (mitigate with an obligation catalog — errors
name the missing fact and the minimal guard), and obligations must float
into `requires` so libraries export them instead of guarding internally —
without contract floating the stance is compositionally unusable.

ZII interplay: length witnesses at zero mean empty (len=0 → nothing to
access); standing couplings must hold at the all-zero value (the existing
"range must include 0" rule, generalized verbatim); facts zero cannot satisfy
(`stride >= 40`) are minted subdomains, not standing invariants. Decode mints
grant exactly the checked predicates and nothing more — firmware semantic
truth is not decodable.

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
4. **The ghost stratum.** Recursive ghost data gated on the existing
   strict-decrease measure (never definitional unfolding — Lean's lesson);
   indexed data via the planned const-param machinery; exactly two universe
   levels until category theory forces more; one quotient lift rule (Lean
   covers quotients with 4 constants + 1 reduction rule). Opens algebra,
   combinatorics, number theory, construction of the reals. QTT/Idris 2 is
   the proof that one calculus hosts erased mathematics beside linear
   runtime resources; Omega's split is cheaper still because ghost values are
   never borrowed and need no layout.
5. **Proof machines as the escape hatch** — state machines run at compile
   time by the existing interpreter, terminating by the existing decreases
   discipline, emitting derivation records the checker validates. No foreign
   "tactic" concept; mathlib hammer data (~37% automatable) calibrates the
   front-line/escape-hatch split.

**The foreclosure ledger** — near-zero-cost mitigations to adopt NOW so the
systems fragment never blocks the math rung:

- Never define fact-truth as "the engine accepts it" (the F*/Dafny trap):
  write every engine rule as a declarative inference rule in the design
  record; "engine accepts" stays a documented under-approximation of
  "derivable", and that rulebook becomes the Stage-3 kernel spec.
- Scope the `as`-sole-mint doctrine to runtime carrier domains; a
  proposition is not a decodable thing.
- ZII and layout obligations never apply to propositions (a zero-inhabited
  proposition is inconsistency by construction); never write "every value
  has a machine layout" into the record — ghost unbounded integers arrive in
  Stage 4.
- Scope NO RECURSION to the runtime stratum — induction IS measured
  recursion, and the decreases discipline is the gate.
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

## 8. Implementation lab agenda (the next conversation)

Ordered rungs, each independently shippable, each with its acceptance driver:

- **R1 — Symbolic atoms:** range endpoints and guard mints go value-vs-value
  (`requires a.cols == b.rows`; `i: u64 [0..items.len]` as requires sugar).
  Engine: DBM atoms already relational; plumbing + surface only. Driver:
  matrix agreement.
- **R2 — Couplings + loans:** default-domain build-out (already settled)
  carrying cross-field couplings; loan-on-witness in the borrow checker;
  store-checker enforcement of couplings; zero-satisfies-coupling ZII rule.
  Driver: `len`-sizes-`payload` data.
- **R3 — Bounded-product rule** in the polynomial engine. Drivers: `y*W+x`
  (unblocks the TASKS.md nonlinear-index entry), `i*stride`.
- **R4 — View parameters + boundary witness mints:** out-params as
  witnesses, decode-minted subdomains, the recast-borrow obligation wired to
  couplings. Driver: the memory-map walk (rides Cathedral M2's recast).
- **R5 — Frames:** preserve-unless-written; `stores` clause at boundaries;
  state-level `requires` + arrival facts; Houdini pass over the engine.
  Driver: dependent facts across sibling-machine calls.

## 9. Open questions (owner)

1. The default-domain declaration surface (already an open pin in ch7) is
   now load-bearing for couplings — priority bump?
2. `stores` as the frame-clause name and its boundary-mandatory rule — right
   call?
3. The ZII generalization (all-zero value must satisfy every standing
   coupling; zero-unsatisfiable facts are minted subdomains) — confirm.
4. Chapter numbering: 23-at-the-end with reading-path placement (chosen to
   avoid breaking ~455 chapter-number references incl. live Cathedral wiki
   links) vs a renumber pass.
5. v1 dynamic-sized storage: views + fixed-capacity buffers only, owned
   runtime-sized values gated on `Region` — confirm.

## Key sources

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
