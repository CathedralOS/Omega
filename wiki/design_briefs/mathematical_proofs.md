# Design Brief: Mathematical Proofs — the Stratum That Wasn't

Drafted 2026-07-18. Status: the constructs marked **settled** below are
owner-settled and reflected in chapters 3/7/8/10/12/18; everything else is
**direction, not decided**. Companion to
[proof_engine_north_star.md](proof_engine_north_star.md) (the automation/kernel
fork), [dependent_types.md](dependent_types.md) (the systems fragment), and
[proof_caching.md](proof_caching.md) (certificates and the succinctness
trilemma). This brief records how the "Lean-competitive" exploration
progressively dissolved into existing constructs — and what genuinely remains.

## 1. The dissolution sequence

The exploration began from the north-star question ("what would
Lean-competitive look like without polluting systems code?") and ended with
almost no new language. Each proposed construct collapsed, in order:

1. **A ghost stratum marker (`proof machine`)** — dissolved. Chapter 10
   already defined proof machines as *ordinary machines used only as
   evidence* ("if a machine is used only to establish facts, it emits no
   runtime code"). Purity is structural (no effects/capabilities); proof-ness
   is contagion from proof-only types plus usage, not a keyword. Dual-use
   machines (pure, total, measured, over machine types) both run and serve as
   engine atoms — most width-bounded theorems never need a ghost twin.
2. **`forall`/`exists` keywords** — dissolved. Universal claims over all
   values were always machine parameters (checked symbolically once).
   Element-wise facts were already element types; **window facts**
   (`maps[0..loaded] in MemoryMap` — subslice syntax as the binder-free
   quantifier, settled) cover prefix/map shapes. Relational facts (sorted,
   distinct) are **predicate machines + one extraction lemma each**; the
   engine holds the quantified fact-shape internally (lemmas.rs reserved
   exactly this shape) and instantiates mechanically at in-scope index atoms.
   A binder-bar form (`|k in 0..loaded| fact`) is parked as pure sugar over
   anonymous hash-named predicate machines — Rust-shaped but tamer (explicit
   bounded binder, zero capture, erased) — built only if relational one-offs
   hurt in practice.
3. **A recursion dispensation for proofs** — dissolved into one language-wide
   rule (settled; see §2).
4. **An `assume` keyword** — deliberately never (see §4).

What genuinely remains as *new*: `boundary data` (the trust keyword's data
face — par-7) and, far rungs, derivation records, the kernel, and reified
goals for tactic-style proof machines. The `unbounded` property DIED in
review: proof-only is computed from structure (recursive data), never
spelled. Final new-surface tally for the whole arc: `<machine M>` +
`boundary data`; zero keywords, zero properties.

## 2. Measured recursion (settled 2026-07-18 — amends the NO RECURSION directive)

The directive's original form ("no cycles") was a mechanism ban standing in
for a principle now stated directly: **a cycle without a measure is an
unproven termination claim; a cycle with one is induction.** The owner's own
record left the door open ("we can maybe relax this later"); this is that
relaxation, precisely scoped:

- **Legality**: recursive call cycles (direct or mutual) require `decreases`
  — both strata, all positions. Unmeasured call cycles remain the same hard
  error. Transition loop-backs are untouched: jumps, unmeasured, constant
  stack, free to diverge (servers, game loops). Spelling encodes intent.
- **Lowering**: tail position → the landed loop machinery (zero stack;
  strict classification, mis-classification is a clean error, never a silent
  cost change). Non-tail → a const depth budget proven at entry
  (`measure <= BUDGET`), with the activation region an ordinary
  machine-storage field (`[Frame; BUDGET]` + depth witness) — the
  dependent-types fixed-capacity storage shape, self-hosted on activation
  records. It appears in the layout report; there is no OS stack to smash.
- **Exposure analysis**: the maximum stack is always a static constant
  (budget × frame size, printed); runtime-unbounded depth is *uncompilable*
  (the budget obligation cannot discharge); composition is additive along
  call chains, so the **whole program's worst-case stack is computable at
  build time** — the property embedded/avionics shops ban recursion to
  approximate, obtained here as a theorem. Ch18's no-stack-sizes guarantee
  is strengthened, not weakened.
- Tail-vs-decreases precision (owner probe): `decreases` is the sole
  ungating mechanism; tail was blessed only as a lowering guarantee. Letting
  tail-ness excuse a measure would make legality hinge on syntactic position
  (`* 3` toggling a machine between legal and illegal) — the arbitrariness
  the unification exists to delete.

**AMENDMENT (owner review): runtime non-tail is CUT.** The probe that killed
it: "at one point recursion was banned and we seemed quite contented... now
you're saying that 1 explicit case is load bearing." Unpacking that exposed
a strata conflation in the defense. The load-bearing customer of measured
recursion was always the PROOF stratum — induction — where fact-only
machines erase and no frame, budget, or constant ever exists. Runtime tail
is the loop machinery wearing call syntax. Runtime bounded non-tail was a
corollary taken because it cost one sentence — and it had no current
customer (first-boot, rendering, fs, and the proof arc are all tail- or
loop-shaped; the parser/AML case was a future-customers argument). Owner
verdict, verbatim core: "stack pressure is often hackier than forcing
people to write code that doesn't repeatedly produce stack frames (hence a
language centered around state machines)." A five-million-deep map file is
not a frame-budget problem — you iterate, handle it, and structure the code
so depth lives in data the author sizes; if that storage exhausts the heap,
that is a more accepted risk than stack exhaustion, given the size
difference. Consequences: the frame-budget/cardinality rule is DELETED (and
with it the only hard-coded constant on the runtime recursion surface);
dependent range endpoints are unrestricted (the range is a termination
fact, never a size); lexicographic measures need no non-tail carve-out; the
whole-program stack bound simplifies to the longest chain of an
acyclic-after-lowering call graph. If a real customer ever demands
recursion-spelled depth, the recorded future shape is frames allocated from
a Region — a visible allocation, consistent with the verdict's
heap-over-stack risk ordering. The Lowering/Exposure bullets above stand as
the pre-amendment record.

## 3. The membrane and the Collatz shape

One construct connects strata: a fact justified by citing a proof machine's
contract, instantiated at operands (chapter 10 "Citing Proofs"). The
canonical worked shape is the Collatz walker: the ideal step is defined once
(over `Nat`, or dual-use over `u64`); `step_fits` bounds one step;
the runtime walker's `ensures self.n == collatz_step(n0)` is a **refinement
fact** — the u64 operation *is* the mathematical function on the domain where
witnesses fit. Prove once, embed per width by supplying each width's bound.

Honesty notes that shaped the design: Collatz termination is open
mathematics, so the walker keeps its fuel bound — what proofs delete is
overflow checks and trajectory-faithfulness doubt, not the fuel. The
2^68-verified claim is **not kernel-checked anywhere on earth** (and earlier
generations of that computation had coverage gaps found later); if imported
it enters as evidence class `external-run` — a proven *checker* plus a
recorded *attestation* — never as a re-executed certificate. The useful
recorded theorem is the stopping-time bound (making the fuel-exhausted arm
provably dead below 2^68), not bare termination.

Beyond Collatz, the natural membrane customers are systems-flavored:
open-addressing probe coprimality (gcd ⟹ full slot coverage), bignum/crypto
limb bounds (the HACL* precedent), power-of-two mask identities, CRC
polynomial facts — the "inexpressible without hacks" list.

## 4. Trust: boundary machines (the ten-round collapse)

The trust design went through ten owner-review rounds, and the record of
what died is the argument for what survived. Proposed and killed, in order:
a YAML `Proofs.lock` input (build.omg exists precisely to prevent sidecar
config); a `grant` keyword (grants are Build-API data); a path-denotes-record
pun (context-dependent meaning); a five-class evidence ladder
(`external_run` vs `assumed` had **no distinct compiler action** — taxonomy
cosplaying as grammar); a structured evidence schema, then an optional
evidence field (both re-invented what the *statement itself* can say); and
first-class PCC (re-invented `as`). The test that did the killing, owner's:
**a distinction earns syntax only by earning a compiler action.**

What survived, each with its action:

- **`boundary machine`** — a contract with no body; the proof system's face
  of the existing boundary culture (boundary traits have no evidence
  classes either; trust quality is the auditor's judgment). Legal in any
  package, **inert until granted**. The statement carries all specificity:
  trust the narrow execution claim (`ensures check(cert_#x) == true`, the
  certificate's identity inside the statement) and lift it to the broad
  theorem with a userspace proof machine.
- **Root grants** — `b.accept_boundary<machine M>();` in build.omg (a
  compile-time machine parameter). Libraries request; the root countersigns;
  link fails otherwise. No transitively inherited axioms.
- **The lockfile pin** — statement hash recorded automatically at grant;
  drift fails the build until re-approved. No hand-typed hashes, ever.
- **The engine veto** — refutable boundary statements are compile errors,
  grants notwithstanding.
- **The deferral gate** — "prove later" rows (tooling-written) warn always
  and cannot ship.
- **The trust report** — which conclusions rest on which boundary machines;
  hashes are plumbing, never porcelain (grants by name, report by class).
- **Oracle tripwires** — runtime-decidable boundary claims instrumented in
  proof builds; a violating test run names the machine that lied.

Tiers with distinct compiler behavior, final: **proven** (no declaration) |
**evaluated** (fuel-budgeted build-time run, Merkle-cached) | **deferred**
(unshippable) | **accepted** (boundary machine). Certificates are userspace:
cert = wire data, checker = measured machine, soundness = membrane theorem,
establishment = `evaluated` or an `as` mint through a certificate domain —
the validated-decode pattern applied to proofs. Lean context: its defense is
social (mathlib bans `sorry`; axioms beyond the sanctioned trio are
PR-rejected; nothing structural stops a malicious package) — this design is
that equilibrium made structural, with the attestation tier given an honest
home instead of living in papers.

## 5. Lean-shape triage (what maps where)

| Lean shape | Omega mechanism | Verdict |
|---|---|---|
| `omega`/`linarith`/`ring`/`positivity` | the ambient engine — no invocation | more elegant |
| `induction` + `cases` + `ih` | the state machine itself (dispatch/back-edge/decreases) | more elegant |
| `use`/`obtain` (existentials) | out-params as witnesses | more elegant |
| `decide`/`native_decide` | `evaluated` class, explicit fuel | comparable+ |
| `sorry`/`axiom` | ledger rows, root-granted, unshippable deferral | more disciplined |
| `have`/`calc` | sub-proof-machine calls; DBM does transitivity silently | comparable |
| custom tactics | proof machines over reified goals (far rung) | comparable, terminating by construction |
| `rw`/`simp` sets | **missing** — needs measures + rewrite registration + a termination story | the biggest authoring gap |
| `conv` | absent; matters only for deep algebra | missing, low priority |
| typeclass-search hierarchies | spelled instances (frozen: no unification) | chosen deficit: verbose at CommRing depth, predictable forever |
| nested `∀∃`, `push_neg`, classical | proof-machine statements; kernel-stage | deferred by design |
| term-mode proofs | never — evidence, not terms (erasure for free) | philosophy |

Cost tl;dr (owner-asked): authoring = 1–3 extraction lemmas per relational
predicate, library-side, once (DBM composes instances, so no mathlib-style
lemma zoo); compile = flat closed-rule instantiation under a budget, no SMT
variance (Mariposa's 2.6–5% instability is the avoided cost), Merkle-cached,
contract-modular linear; residual = occasionally naming a ghost index the
engine can't see (one line, deterministic error).

## 5b. Post-spree settlements (owner review arc)

Settled through chat review after the documentation spree; chapters updated:

- **`decreases ... in <range>`** (ch3/ch9): the measure declares its range;
  the frames clause and a `within` keyword both died. Frame capacity = the
  range's **cardinality**; the well-foundedness floor = the range's start
  (non-zero floors legal — no re-zeroing ceremony). Dependent endpoints
  (witness-named, ch12 machinery: pinned across the cycle, re-proven at
  back-edges) are tail-only in v1 — a runtime cardinality would be a
  runtime-sized frame region. *(Amended, par-2: runtime non-tail is cut —
  cardinality-as-capacity and the endpoint restriction are both dead; the
  range is a termination fact only.)* Linear monotone ranges are the only primitive;
  lexicographic is the only extension (unbounded resets: omega-squared does
  not order-embed into omega — bounded components flatten as `m*B + n`,
  which is the R3 product lemma); everything else is a measure the user
  writes. Beyond-epsilon-zero termination (Goodstein/Hydra) is unprovable in
  arithmetic for everyone and permanently out of scope.
- **`measure` survives as sugar for a named ordering satisfier** (ch10/ch14).
  A dissolution into machine-returning-struct was proposed and owner-killed
  (reflexive data; Rust's sort_by_key-returning-tuple minus tuples). The
  mechanism underneath: ch14's landed machine-level `satisfies` clause +
  name-freedom (machine name may differ from the trait's method name) =
  **named satisfiers**, multiple per type, prover-transparent because the
  ordering trait is key-shaped (rank -> well-founded carrier, never
  cmp-shaped). Selection: spelled where concrete meets abstract (generic
  instantiation; dyn coercion carries the choice in the descriptor —
  settled spelling: `as &dyn Card::PowerOrder`, decaying to `&dyn Trait`);
  elide-when-unique, loud-when-plural. Rust buys obviousness globally
  (coherence, hence one Ord per type and the newtype smear); Omega buys it
  locally.
- **Machine parameters, no Fn hierarchy** (ch13). `<machine M>` binds a
  symbol at the spelling site, monomorphized, gone by codegen; three
  customers already wait (decreases' selector — shipped; satisfier
  selection; sort_by-shaped APIs). Fn/FnMut/FnOnce manage *implicit
  capture*, which Omega machines don't have: signatures = where-machine
  grammar (landed), discipline = receiver modes, closures = machine
  instances (fields are declared captures; borrow modes are field types),
  erasure = dyn, threads = move-at-spawn + the `send` property. Consuming
  receivers ride the cleanup arc; anonymous machine sugar is parked
  (binder-bar verdict: pure sugar, wait for pain).

## 6. Open questions (for the next session)

1. ~~Rewrite registration~~ **SETTLED: fact injection is the default;
   registration is PARKED.** Carrying a theorem to a site is an ordinary
   statement call (a fact-only machine invoked for its `ensures`, which
   enters flow facts and erases at codegen — the composition rule's normal
   job). Rationale (owner): in the co-authored-code era the tradeoff simp
   optimized has inverted — verbosity is cheap (machines type the ceremony)
   while invisible context is expensive (humans and models read the file
   cold); an explicit lemma call keeps the proof structure in the text,
   where an import-activated rewrite proves things for reasons visible only
   in the import list. Mitigations shipping with the default: the failure
   diagnostic names the missing lemma by shape-match ("note: mask_is_mod
   rewrites `_ & (_ - 1)` and is not in scope"), and derivation records name
   the lemmas each obligation consumed. Dafny is the existence proof that
   this mode works at scale. The registration design is retained here for
   un-parking if a math-library corpus ever hurts at mathlib scale: rules
   must shrink under ONE engine-canonical term order (measured rewriting —
   modular, packages compose), conditional rules re-discharge side
   conditions per site, activation is import-scoped (nothing ambient),
   direction and eligibility are declared at the definition (`[rewrite]`,
   the satisfies-not-structural-matching precedent).
2. ~~The `by` keyword's fate~~ **SETTLED — `by` is deleted, zero users.**
   The dyn coercion respelled as `&card as &dyn Card::PowerOrder` (owner):
   dyn-over-a-satisfier-path, decaying immediately to `&dyn Trait`; implicit
   when unique; the explicit chain (`as &dyn Sat as &dyn Trait`) is valid by
   composition (coercion + identity recast), never designed. Final keyword
   tally for the entire proofs/trust arc: **zero** — boundary, dyn, as,
   satisfies, decreases, in were all already in the building.
3. ~~The `unbounded` property spelling~~ **SETTLED: no property exists.**
   Proof-only is computed, never spelled — a type is proof-only iff it is
   recursive (directly or mutually) or contains a proof-only field. Owner:
   "the violation is already opting into the semantics"; the marker earned
   no compiler action (the house test), and Omega is cleaner than Rust here
   — with no Box, recursive data has no runtime meaning it could have
   accidentally intended. Core roster: `Nat`, `Seq<T>`, `Bag<T>`, `Rat`
   (every finite float is a dyadic rational — the embedding is exact; float
   verification is Rat-vs-Rat error bounds, the Gappa road; posits and any
   other format are libraries, f32's FPU ops are the boundary-trusted
   special case — the language is never float-aware), `Int` on demand
   (order has no floor; `decreases` measures stay Nat-valued or
   range-floored). The proof views (Seq/Bag/Range) dissolve from
   compiler-known atoms into ordinary core data + lemmas. `Real`: par-7.
4. **Deferral ergonomics**: compiler-writes-the-declaration tooling vs an
   in-source attribute paired with a mandatory row.
5. **Reified goal type** for tactic-style proof machines — far rung; shares
   the declaration-reification machinery sketched for `M::proof`-style
   synthesized records (Equatable/ZII synthesis precedent).
6. Derivation-record and kernel formats — north-star stages 2–3, unchanged.
7. **Engineering pins**: the `<machine M>` parameter kind through the
   generics pipeline; dyn descriptors carrying satisfier identity; the
   in-range decreases obligation in MR3.

## 7. The Real arc (settled through owner review)

`Rat` and `Real` are different animals: a Rat is a pair of Ints — finite
data; a Real is infinite information, representable only as the
approximation process itself (a Cauchy stream of Rats), two streams equal
when they converge together. Reals are symbols everywhere — even Lean's
`Real` is noncomputable by design; proofs push algebra, never digits;
computation lives in Rat/float land, connected by proven error bounds.

Settled:

- **`boundary data Real;`** — a type with no definition, as a boundary
  machine is a contract with no body (owner: "data and code can both be
  'trust me bro'"). This is the one proof-only case structure cannot infer
  (an axiomatized carrier has no structure), and it resurrects no marker:
  an undefined type IS a trust object, so it reuses the trust surface. Ops
  are ensures-less boundary machines (symbols; claim nothing; no grant);
  axioms are boundary machines with ensures (one accepted-tier row each).
  Opacity is mathematically justified: any two complete ordered fields are
  isomorphic — the rulebook pins the carrier uniquely.
- **Constructed Real is the end-state** (owner: "I sorta trust the Lean gut
  instinct"). Axioms can be jointly inconsistent, and one contradiction
  proves everything; construction inherits consistency from the kernel. The
  axiom package ships anyway as interim scaffolding — users get Real specs
  immediately, the trust report shows the honest rows — and retires by the
  settled boundary-upgrade swap. Engine note that decided the staging: the
  normalizer keys on native `==`, which axioms preserve; a setoid
  construction (custom equivalence, no quotients) would wall Real off from
  the polynomial/DBM machinery, so quotients are the required gadget, not
  setoids.
- **No universe ladder — parked with a trigger.** The Russell/Girard
  paradox needs types as values; Omega never reifies types. Quantification
  over predicates = machine-parameter schemas, monomorphized per use (the
  PA-induction-schema precedent) — predicative, paradox-immune by
  construction, no Type-tower to design or audit. Cost: proofs whose
  STATEMENTS treat all-predicates as one completed object (deep
  set-theoretic foundations, not working analysis — the construction sans
  universes is Weyl's 1918 predicative program, mechanized). A universe
  tower would be foundation surgery comparable to the entire entailment
  engine, with a documented history of soundness bugs in Coq/Agda/Lean.
  Trigger to revisit: full-mathlib replay becoming a language goal, and
  better surgeons (owner: "we'll wait for LLMs to get better so they can
  do this surgery more precisely"). Parks next to reified goals.
  Migration insurance, pinned now so the door stays a door: universes, if
  ever, are PROOF-STRATUM-ONLY — runtime types stay static forever, and no
  future feature may depend on "types are never values" for soundness
  elsewhere. Under that fence the move is additive: predicative proofs are
  valid impredicative proofs (a schema instance IS the universe theorem
  instantiated), so the corpus ports verbatim; the kernel is built
  alongside the engine, not inside it.
- **Fact-position operators route by operand type**: Nat/Int/Rat compute
  (engine bignum); Real rearranges (the glyph maps to the declared symbol;
  the normalizer works under the ring axioms — the polynomial layer is
  ring-generic). Real never exists at runtime, so it never queues behind
  the runtime operator-overloading question (posits et al. — that arc
  stays open, and it is the one genuine gap the posit exploration exposed).
- **One lockfile (owner).** Trust receipts and package resolution share
  the same machine-written lockfile — "one to rule them all rather than
  keeping it disjoint." build.omg stays the only file a human authors;
  the lock is the compiler's receipt: statement hashes of granted
  boundaries beside pinned package identities. The Cargo.toml/Cargo.lock
  split, with the receipt side unified.

Open sub-decisions (owner-gated, dependency order):

1. ~~Classical logic~~ **SETTLED (owner sided with the Coq
   philosophy).** LEM is an ordinary core boundary machine; NOTHING is
   granted by default, not even logic — "grants flow from the root" stays
   absolute with zero exceptions. Project templates carry the one grant
   line; constructivists delete it; the trust report shows whether a proof
   tower is classical or constructive. Why Lean chose the other default,
   for the record: audience — mathlib's working mathematicians are
   classical to the bone, while Coq's proofs-as-programs culture needed
   constructivity for extraction (classical axioms break "a proof of
   exists-x computes x"). Omega is Coq-shaped for a systems reason:
   what-did-you-assume is the brand. Note LEM is not choice: choice is the
   stronger axiom and implies LEM (Diaconescu); Lean ships choice and
   derives LEM. A century-old philosophy-of-math war absorbed as a
   lockfile row.
2. ~~Machine-parameterized proof data~~ **SETTLED**: `data
   CauchySeq<machine S>` — ch13 machine params extend from machines to
   data declarations (monomorphization everywhere, no machine-valued
   fields, consistent with the runtime ban). Nested schemas (machine
   parameters whose signatures take machine parameters) remain the one
   genuinely new capability the construction demands.
3. ~~Quotient spelling~~ **SETTLED: `%`, corrected from the slash.**
   `data Real = CauchySeq % converges_together;` — the owner caught the
   `/` misread ("it looks like we're dividing by a keyword"): `/` was
   math transcription (T/~), where `%` says wrap-by-equivalence in the
   audience's native glyph — wrapping arithmetic IS the familiar quotient.
   Honesty correction on the record: the "dual `in` derivation form"
   (`data X = Y in D`) cited in chat was extrapolation, NOT landed — ch8's
   landed surface is `in`-annotated type expressions (`&[u8] in Utf8`)
   plus `as` mints. The quotient therefore introduces ONE new declaration
   face, not two: bodyless `data Name = <type expression>;` (the const
   assignment precedent), with `%` as the single new type-expression
   form. The four pieces land on existing constructs: equivalence = three
   ordinary lemma obligations (refl/symm/trans); mk = the `as` mint,
   CARRIER-ONLY (`seq as Real`; `42 as Real` rejects — that road runs
   through Rat and a constant stream); lift = the respect-ensures gate
   (`equiv(a,b) => f(a) == f(b)` — bucket-safe machines callable on the
   quotient, others reject); sound = engine congruence. The buckets span
   the machine-parameterized family — pi-via-Leibniz and pi-via-Machin
   are different `CauchySeq<S>` instantiations in one bucket (that is the
   point) — so `converges_together` is itself a nested-schema machine:
   one more N7 customer. Documented ch10 Proof-Only Data.

With these, the Real arc is OWNER-COMPLETE: N1-N8 carry no remaining
design gates.

The corpus (well-definedness of the ops, ordering, completeness) is
mathlib-scale ceremony: long, LLM-parallel, zero runtime-compiler conflict
(nothing lowers), and the ultimate dogfood — it stress-tests measured
recursion, schemas, quotients, and bignum simultaneously.
