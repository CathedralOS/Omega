# Design Brief: Proof-Engine North Star — Obsoleting SPARK, Rust, and Lean

Drafted 2026-06-19. Status: **settled architectural direction with staged
migration.** Records the long-range target for Omega's proof layer and the
architectural fork it forces. Landed source and terminal-Psi rules remain
governed by the language guide and `TASKS.md`; this brief states the endpoint
those increments must converge toward.

Update 2026-08-02: the terminal-Psi architecture has now settled the fork in
favor of a small trusted kernel plus untrusted/certifying automation. The
initial Psi-owned scalar proposition core, structural kernel, total primitive
judgments, certificate envelope, and sealed admission validator are live. The
broader quantified proof language and automation-to-certificate bridge remain
long-range work; see
[Terminal Psi, Fuel, And Resource Provisioning](canonical_ir_fuel_and_resource_provisioning.md).

Update 2026-08-04: nominal `proposition` declarations establish `Prop` as the
formula universe. The long-range core is one typed framework with distinct
value, proposition, and effectful-computation judgments, explicit relevance,
validity scope, and trust provenance. This is a semantic north star; it does
not retroactively replace the current structural proof-only classification or
make the effectful computation judgment an implementation prerequisite.

## The ambition

Omega should make whole *classes* of existing tools redundant inside one
systems language:

- **Rust** — memory/concurrency safety, but by *proof* (ownership + borrows +
  logic errors proven away) rather than by a borrow-checker's conservative
  approximation.
- **SPARK** (Ada + `gnatprove`) — contract-checked systems code, discharged
  automatically by an SMT backend.
- **Lean / Coq / academic proof languages** — serious mathematics: "at some
  point we want to port a huge swath of mathematical proofs to Omega."

The first two are *near*; the third is the long pole. The point of this brief is
that they are not three separate projects — they are three rungs on **one**
proof engine, and the rung you can reach is set by a single architectural
choice.

## Primer: what a proof actually is

By Curry–Howard, **a proof of a proposition `P` is a value whose type is `P`** —
a *term*. A small **trusted kernel** does exactly one job: type-check that term
against the proposition. The kernel is the entire trusted base — a fixed set of
inference rules plus a handful of **axioms** (the unproven assumptions, e.g.
excluded middle / choice). Everything else can be wrong without compromising
soundness, as long as the kernel re-checks the final term.

**Tactics** exist because writing those terms by hand is brutal. A tactic is an
*untrusted metaprogram* that inspects the current goal and emits term fragments:

- structural — `intro`, `apply`, `induction` (one inference step);
- rewriting — `simp`, `rw` (apply known lemmas / equations — "shortcuts");
- **decision procedures** — `omega` (linear integer arithmetic), `ring`
  (commutative-ring identities), `linarith`. These discharge a whole goal
  automatically.

The load-bearing property: **tactics are untrusted.** A buggy tactic cannot
prove a falsehood, because the kernel re-checks the term it produced. So you can
pile on arbitrarily clever automation without growing the trusted base. (Axioms
are the opposite — they're what you *don't* prove; minimize them.)

The key observation for Omega: **the entailment engine we already have is a
decision-procedure tactic** — the same category as Lean's `omega`/`ring`. The
difference from Lean is purely architectural, and that difference is the fork.

## The fork

| | Automation-as-base | Kernel + tactics |
|---|---|---|
| Examples | **Omega today**, F\*, Dafny, SPARK | Lean, Coq, Agda |
| Trusted base | the whole prover/SMT engine (large) | a tiny kernel (small) |
| Proof terms | none — the engine decides directly | every proof is a kernel-checked term |
| Ergonomics | fully automatic where it works | tactics/term-mode for the hard parts |
| Reach | only what the procedures decide — **no arbitrary quantified / higher-order math** | **all of mathematics** |
| Failure mode | engine bug = silent unsoundness | kernel is small enough to trust; tactic bugs caught by re-check |

Omega is unambiguously in the left column today, and doing it well. The left
column is *exactly* what "kill SPARK / Rust" needs. It is *fundamentally
incapable* of "kill Lean" — undecidability means no decision procedure covers
all of math, and with no kernel there is nothing to check a human-supplied proof
against.

## The synthesis to build

Neither column alone is the target. "Kill Lean inside a systems language" does
**not** mean adopting Lean's manual misery — it means building the synthesis
that neither camp ships cleanly:

1. **A tiny trusted kernel** so arbitrary proofs are *expressible and checkable*
   (this is what unlocks general math and shrinks the trusted base).
2. **Automation as the front line** so the basic 95% — bounds, overflow, ranges,
   ring identities, bounded induction — is discharged with *zero* tactics, the
   way the entailment engine already does it.
3. **An escape hatch to an explicit proof** for the genuinely-hard 5% that
   automation can't crack; the kernel checks it. An SMT-style procedure can even
   emit a kernel-checkable certificate, so automation and kernel compose.

This is strictly better than Lean (far less hand-proving) and strictly better
than pure-SMT/SPARK (it can do the hard cases at all, with a smaller trusted
base). F\* is the nearest existing point on the map (SMT automation over a
dependently-typed, kernel-checked core) — the closest prior art to study.

## One typed core, three judgments

The destination is one calculus, not several unrelated proof mechanisms:

| Judgment | Subject | Runtime meaning |
|---|---|---|
| `Type` | mathematical and runtime values | depends on relevance and layout |
| `Prop` | formulas that may hold about values | no runtime representation |
| computation | effectful machines producing values or proofs | carries effects, work, failure, suspension, and termination |

`data` constructs values in `Type`. `proposition` constructs formulas in
`Prop`. A proof is an inhabitant of a proposition. An ordinary checked machine
may construct values or establish proofs; `requires` and `ensures` elaborate to
erased proof flow at the machine entry and terminal edges. Trait laws,
termination facts, domain facts, quotient laws, and conservation equations
therefore share one proposition/proof account even when their source surfaces
remain specialized and ergonomic.

This unification preserves three distinctions:

- an object such as `Nat` is not a claim about that object;
- a proposition is not the particular evidence that establishes it; and
- a proof is not a decision procedure returning `bool`.

The `Prop` universe is internal and proof-only. Source proposition families are
not runtime values, fields, layouts, or machine result carriers. A Boolean
expression in a contract is the decidable special case: it denotes the
proposition that the expression evaluates to `true`.

## Evidence dimensions

Proof erasure is not the whole evidence model. Every retained fact is described
along independent dimensions:

- **relevance:** whether it contributes runtime representation or behavior;
- **multiplicity:** whether it may be copied, must be consumed, or may be
  discarded;
- **validity scope:** timeless, borrow-scoped, entry-scoped, state-versioned
  and invalidated by intersecting writes, or lease-scoped; and
- **provenance:** derived, certified, admitted, and the exact authority/evidence
  chain that supplied it.

Omega's existing `[copy]`/affine/`[linear]` rules provide much of the
multiplicity substrate, but erased relevance remains a separate judgment: a
proof may be erased while still linear or scoped. Provenance attaches to the
evidence, not to proposition identity, and composes through every proof so a
deployment profile can reject a proof closure containing unacceptable
admissions.

## Witnesses and elimination

A fact-only proposition hides all proof identity. A witness-bearing nominal
proposition publishes one opaque evidence interface. Selected carrierless
conformance is one representation of an inhabitant of that proposition, not a
parallel logical mechanism. The normalized interface is fingerprinted public
proof content; changing it is a breaking proof-API revision while the nominal
proposition symbol remains stable.

Opening proposition evidence is allowed only within proof-relevant or erased
computation. A witness that must influence runtime computation belongs in an
ordinary `Type`-level dependent pair whose relevance is tracked explicitly;
erasure alone does not authorize eliminating a `Prop` inhabitant into runtime
`Type`.

## Migration boundary

The current rule that recursive/non-layoutable mathematical data becomes
proof-only remains live until explicit relevance replaces it. During migration,
an explicit relevance annotation takes precedence; structural classification
is then legacy inference for unannotated declarations. The later effectful
computation judgment must account for Omega's states, transitions, effects,
suspension, failure, work, and multiplicity rather than pretending machines
are pure dependent functions. Neither migration blocks proposition families,
terminal-Psi proof identity, or the present certificate kernel.

## Where Omega is today (grounded)

The engine is real and past where systems languages stop
(`compiler/psi-rs/semantics/psi-validation/src/contract_entailment.rs`,
`wiki/proof_engine_roadmap.md`):

- canonical polynomials over atoms (folding, congruence, distributivity);
- a difference-bound matrix with transitive closure (order reasoning, vacuity);
- a correlated-power interval evaluator (range sums, squares, euclidean mod);
- directed substitutions from `requires` equations;
- **induction (L7)** — recursive contracts + `terminates by`, with the induction
  hypothesis gated on a per-call-site strict-decrease discharge (the
  well-foundedness that makes assuming the smaller instance sound).

The L0–L7 ladder is discharged: constant arithmetic, order transitivity, linear
range sums, congruence, commutativity, nonlinear square ranges, antisymmetry,
remainder ranges, inductive accumulator theorems.

The **known ceilings are exactly the automation/general-math boundary**:

- **no quantifiers** in contracts (`forall`/`exists` are parse errors);
- proof views (`Seq`/`Bag`/`Range`, `Sorted`) parse but have no semantics;
- `Real` approximation is an open design question;
- the legacy source entailment engine emits no terminal-Psi certificate, so it
  remains trusted automation on that path. The initial Psi kernel is live but
  not yet connected to source obligations or the broader theorem vocabulary.

## The three kills, sequenced

- **SPARK — near, mostly hardening.** Omega is already SPARK-shaped (first-class
  contracts + automated discharge) and arguably ahead (built-in induction,
  proof-oriented from the ground up rather than bolted onto Ada). The S4
  narrowing work and domains-over-carriers are this rung: make more obligations
  discharge automatically.
- **Rust — in progress.** Ownership/borrows + logic-errors-proven-away
  (panic-as-effect) + exact-arithmetic-by-default. This is the systems-safety
  story already being executed.
- **Lean — the long pole.** Still needs quantifiers, a much broader proposition
  and proof-term vocabulary, and the automation-to-certificate bridge into the
  now-live small Psi kernel. This is most of what Lean *is* — a multi-year arc.

The friendly part of the sequencing remains: the automation-first base delivers
SPARK/Rust-class value while the initial kernel grows, and **the kernel can
become the backstop without discarding the automation** — they compose
(automation tries first; kernel-checked explicit proof catches the rest). So the
end-state is coherent and incremental, not a rewrite.

## The down-payment, and what it buys

Three open threads are secretly the *same* problem — carrying a proven fact
through code without re-proving it:

- **arithmetic-range** (S4 narrowing → return `Wrapping` ops to Exact),
- **text validity** (`[u8]::Utf8` → retire the `String` vestige),
- **recoverable errors** (a "fallible" fact on a sum → strict-use + inferred
  failure surface, ch16).

All three want **domains/facts that attach to and propagate over carriers**
(scalars, `Slice<u8>`, sums) — the design exists in ch8 ("one generic level up"
from `domain i32::Degrees`), the implementation is scalar-only today. Building
that fact-propagation substrate is the highest-leverage near-term investment: it
pays down all three at once and is the same machinery the automation front line
runs on.

**Architecture frozen (decision 18 / ch16, 2026-06-19).** The substrate is now a
named target: ONE **unified flow-sensitive fact catalog** threaded through the
CFG — the borrow checker, the decision-17 interval engine, and sum case-narrowing
converge into a single carrier-generic analysis (scalar→interval, sum→which-case,
slice/`[u8]`→length+encoding, ref→validity), narrowed by guards and case
partitions via intersection. Cross-call propagation is **modular and
contract-mediated** (prove `requires`, assume `ensures`), with contracts inferred
within a compilation unit and written only at boundaries — chosen over
whole-program context-sensitivity precisely so separate compilation survives,
which is also the SPARK-rung architecture. ch16's recoverable-error model is the
first concrete customer: a success case's `ensures` fact is inherited by the
handling arm. v1 fact-kinds: intervals (done) + which-case + slice-length.

## Remaining research questions for the Lean rung

1. **Logic surface:** what fragment of quantification do we admit, and how is it
   discharged — bounded instantiation (stays automated) vs general (needs the
   kernel)?
2. **Kernel growth:** the initial terminal-Psi kernel checks typed scalar
   propositions, structural implication/conjunction proofs, and total closed
   judgments. Which additional term constructors and rules are necessary for
   quantified mathematics while keeping the trusted core small?
3. **Certificate bridge:** can the existing engine emit kernel-checkable
   certificates (so automation stays the front line under the kernel)?
4. **`Real` / analysis:** the proof-side Cauchy/evidence/quotient construction
   and dedicated `proposition` surface are settled; implementing the
   proposition-family/index-telescope fragment gates its
   implementation, while the runtime approximation-policy surface remains
   open.
5. **Trust migration:** which existing automated judgments become total kernel
   primitives, which emit certificates, and which remain explicitly admitted
   while the terminal-Psi bridge is incomplete?

None of these block the near-term work; they're the gates on the long pole, and
this brief exists so they're chosen deliberately when the time comes.
