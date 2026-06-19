# Design Brief: Proof-Engine North Star — Obsoleting SPARK, Rust, and Lean

Drafted 2026-06-19. Status: **direction, not a decided plan.** Records the
long-range target for Omega's proof layer and the architectural fork it forces,
so the choice is made on paper rather than drifted into. Nothing here is frozen;
the near-term work (S4 narrowing, domains over carriers) is committed and lives
in TASKS.md — this brief is the endpoint that work aims at.

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

## Where Omega is today (grounded)

The engine is real and past where systems languages stop
(`compiler/semantics/omega-validation/src/contract_entailment.rs`,
`wiki/proof_engine_roadmap.md`):

- canonical polynomials over atoms (folding, congruence, distributivity);
- a difference-bound matrix with transitive closure (order reasoning, vacuity);
- a correlated-power interval evaluator (range sums, squares, euclidean mod);
- directed substitutions from `requires` equations;
- **induction (L7)** — recursive contracts + `decreases`, with the induction
  hypothesis gated on a per-call-site strict-decrease discharge (the
  well-foundedness that makes assuming the smaller instance sound).

The L0–L7 ladder is discharged: constant arithmetic, order transitivity, linear
range sums, congruence, commutativity, nonlinear square ranges, antisymmetry,
remainder ranges, inductive accumulator theorems.

The **known ceilings are exactly the automation/general-math boundary**:

- **no quantifiers** in contracts (`forall`/`exists` are parse errors);
- proof views (`Seq`/`Bag`/`Range`, `Sorted`) parse but have no semantics;
- `Real` approximation is an open design question;
- no trusted kernel / proof-term layer at all — the engine *is* the trusted base.

## The three kills, sequenced

- **SPARK — near, mostly hardening.** Omega is already SPARK-shaped (first-class
  contracts + automated discharge) and arguably ahead (built-in induction,
  proof-oriented from the ground up rather than bolted onto Ada). The S4
  narrowing work and domains-over-carriers are this rung: make more obligations
  discharge automatically.
- **Rust — in progress.** Ownership/borrows + logic-errors-proven-away
  (panic-as-effect) + exact-arithmetic-by-default. This is the systems-safety
  story already being executed.
- **Lean — the long pole.** Needs the two ceiling-movers: **quantifiers** in the
  logic, and eventually **a small trusted kernel that checks proof objects**, so
  generality and soundness coexist. This is most of what Lean *is* — a multi-year
  arc.

The friendly part of the sequencing: the automation-first base delivers
SPARK/Rust-class value *long* before the kernel exists, and **the kernel can be
added later as a backstop without discarding the automation** — they compose
(automation tries first; kernel-checked explicit proof catches the rest). So the
end-state is coherent and incremental, not a rewrite.

## The down-payment, and what it buys

Three open threads are secretly the *same* problem — carrying a proven fact
through code without re-proving it:

- **arithmetic-range** (S4 narrowing → return `Wrapping` ops to Exact),
- **text validity** (`[u8] in Utf8` → retire the `String` vestige),
- **recoverable errors** (a "fallible" fact on a sum → strict-use + inferred
  failure surface, ch15).

All three want **domains/facts that attach to and propagate over carriers**
(scalars, `Slice<u8>`, sums) — the design exists in ch8 ("one generic level up"
from `domain i32::Degrees`), the implementation is scalar-only today. Building
that fact-propagation substrate is the highest-leverage near-term investment: it
pays down all three at once and is the same machinery the automation front line
runs on.

## Open questions (to decide before the Lean rung)

1. **Logic surface:** what fragment of quantification do we admit, and how is it
   discharged — bounded instantiation (stays automated) vs general (needs the
   kernel)?
2. **Kernel:** what is the trusted core's term language and rule set? How small
   can it be while still expressing the math we want to port?
3. **Certificate bridge:** can the existing engine emit kernel-checkable
   certificates (so automation stays the front line under the kernel)?
4. **`Real` / analysis:** the open `Real` semantics question gates any
   continuous mathematics.
5. **Trust story:** is "the engine is the trusted base" acceptable for the
   SPARK/Rust era, with the kernel introduced only when the Lean rung is taken?

None of these block the near-term work; they're the gates on the long pole, and
this brief exists so they're chosen deliberately when the time comes.
