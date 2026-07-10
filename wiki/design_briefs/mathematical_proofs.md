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

What genuinely remains as *new*: one data property (`unbounded`-shaped) for
proof-only types, the evidence-class ledger, and — far rungs — derivation
records, the kernel, and reified goals for tactic-style proof machines.

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

## 4. Evidence classes and assumption discipline

`Proofs.lock` is one ledger, one row shape, five classes (engine /
derivation / evaluated / external-run / assumed), rows upgraded in place as
machinery arrives. The certificate-size fear is bounded by the
proof_caching.md trilemma: our engine's derivations are proportional to
statements (deterministic, no search transcripts); the blow-up regime is
unstructured exhaustion, which routes to checker-proof + attestation, never
trace-shipping.

Assumptions are treated as *worse than `unsafe`* because false facts are
anti-local (ex falso propagates through the engine into modules that never
mentioned the axiom). Hence: no inline `assume` exists; rows are
boundary-shaped (named authority + artifact — "assume 0 = 1" is
syntactically homeless); grants flow from the root (libraries request, the
top package signs — no transitively smuggled axioms); the engine vetoes
refutable assumptions; deferral-class rows cannot ship; the trust report
names which conclusions rest on which rows; and the interpreter oracle
tripwires assumed runtime-decidable facts in proof builds. The honest-path
cost gradient is deliberate: guard < mint < prove < accept.

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

## 6. Open questions (for the next session)

1. **Rewrite registration** (the `simp` analog): surface for admitting a
   proven equation to the normalizer; termination of rule sets — wants an
   Omega-native answer (`decreases` on rule application?). The first thing a
   proof author reaches for once measures exist.
2. **Budget spelling** for non-tail recursion (the const depth bound clause).
3. **The `unbounded` property spelling** and which proof-only types ship in
   core (`Nat`, `Int`, `Seq`?).
4. **Deferral ergonomics**: compiler-writes-the-ledger-row tooling vs an
   in-source attribute paired with a mandatory row.
5. **Reified goal type** for tactic-style proof machines (the fact AST as
   proof-stratum data) — far rung, but its shape gates the extension node.
6. Derivation-record and kernel formats — north-star stages 2–3, unchanged.
