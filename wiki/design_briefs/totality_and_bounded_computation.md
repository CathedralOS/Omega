# Design Brief: Totality, Bounded Computation, Panic-as-Effect

Scouted 2026-06-15. Status: DIRECTION — a language-design stance, not yet formalized. Syntax provisional.

## The goal

"If it compiles, you're pretty damn sure it works." The sharper, achievable form of that slogan:

> **No unbounded computation is invisible.** Every loop and recursion declares its bound; every panic is a declared effect. So if it compiles, you know its worst-case *and* its failure modes — by construction.

This is the total-functional-programming dream (Idris, Agda, Dafny, F*), aimed at being shippable rather than academic.

## 1. Explicit bounds, not "prove termination"

Do **not** mandate "prove this halts." Mandate "**every loop/recursion carries an explicit bound.**" The bound *is* the decreasing measure, so termination falls out by construction — and this beats a termination-proof mandate on three counts:

- **Decidable.** "Does this loop have a bound?" is a syntactic check; "does this loop terminate?" is the halting problem. You never get stuck unable to prove a *true* thing.
- **Un-cheesable.** `loop 0..u64::max` does not sneak past as "total" — the absurd bound is *in the source*, lintable, and (in strict contexts) requirable to be justified / tied to input size.
- **It is the WCET budget.** The declared bound × per-iteration cost is the worst-case work, made visible.

Genuinely-partial computation is not *forbidden*, only made **explicit** via a fuel/budget parameter:

```omega
// Collatz termination is an OPEN math problem — you cannot prove it total.
// The honest, total-by-construction form carries a budget and is honest about
// not knowing. (Provisional syntax.)
machine collatz(n: u64, budget: u32) -> Converged(steps: u32) | OutOfBudget { ... }
```

So an interpreter is `interp(prog, fuel) -> Result | OutOfFuel`; a solver is bounded; nothing is banned — *hiding* unboundedness is.

## 2. Panic as a default-deny declared effect

Most "panics" are *logic* failures — index out of bounds, unwrap-of-none, arithmetic overflow. These are **proved away by default**: the contracts that prove the index in range / the option is `some` / no overflow *are* the discharge, so a default program is **panic-free by construction**.

What remains is genuine unrecoverable failure (OOM, explicit `abort`). That becomes a **declared effect** — opt-in, propagating up the call graph exactly like other effects (and like the bound/totality contract). A function that can abort says so in its signature, or it cannot. "Panic is a contract you turn on" = "panic is a default-deny effect."

## 3. WCET / bounded-runtime (exploration)

Termination ≠ short. A stronger property — *prove a bounded worst-case runtime* (WCET) — would let RT-critical code run **un-preempted to a deadline** (consumed by Cathedral's scheduler gradient, `Cathedral/wiki/design/part_2_components/01_scheduler_and_resources.md`). But WCET bottoms out at the *hardware* timing model (caches, pipelines — the same below-the-model wall as constant-time), and is practical only for small, carefully-written RT code. Flagged as explore, not committed.

## 4. The general-language implication: every library author's trilemma

Omega is a **general systems language**, not only Cathedral's OS substrate — so the prove-or-bound choice is a *daily* concern for ordinary library authors, not a kernel curiosity. Ship a Collatz walker and you hit it immediately, because Collatz has no known termination proof:

- **Prove it** — ship a small inductive certificate *if the property is provable*. For Collatz none exists; for most ordinary code the bound below *is* the proof.
- **Bound it (recommended default)** — `collatz(n, budget) -> Converged | OutOfBudget`. The bound *is* the decreasing measure, so this is **genuinely total, not a lie** — it provably terminates *within budget*. The only cost is an honest API: a budget parameter and an `OutOfBudget` branch the caller handles.
- **Declare it partial/divergent** — carry a *may-not-terminate* effect (§2) that propagates to callers. Honest, but it **infects every caller's signature**, and it *overstates* the problem: an unbounded Collatz walker is not "non-terminating," it is **termination-unproven** — which the bound expresses more precisely and without the contagion.

The tax, felt equally by OS code and a random utility library: **you cannot ship "unbounded-but-trust-me-it-terminates."** Totality has no free lunch for unproven termination. So the guidance is **bound over mark-partial**: the budget form says "it terminates, I just can't prove the unbounded version" — true *and* ergonomic — whereas the partiality effect says "may diverge" (technically true in the unprovable tail, practically misleading) and taxes every caller. The transmissibility trilemma in `proof_caching.md` (small inductive cert vs. verified-but-large vs. cryptographic argument vs. attestation) is the *distribution* face of this same author-facing choice.

## The cost, and the bet

Total-functional languages have always been *harder to write* — discharging all the obligations is the proof burden that kept them academic. The Cathedral-era bet: **LLMs author the bounds and proofs.** The thing that sank total languages (too hard for humans) is plausibly exactly what LLMs make tractable — and `proof_caching.md` keeps the resulting heavy checking affordable in the edit-compile loop. This stance is only shippable *because* of that bet; name it rather than hide it.

## Open questions

- **How bounds are expressed** — bounded `for` ranges, fuel/budget params, decreasing measures, or all three? The minimal surface.
- **How strict, and where** — is "every loop bounded" global, or relaxed (behind a declared effect) in some contexts? Must the bound be *justified* (input-tied), or merely *present*?
- **Totality vs productivity split** — handlers/pure functions are *total* (terminate); reactive loops are *productive* (run forever, always progress). What is the syntactic/contract distinction, and how does it propagate down the call graph?
- **Panic-effect surface** — exact effect name(s), how `abort`/OOM differ, how it composes with the existing effect ceiling (chapter 18).
- **Interaction with WCET** — does a bounded-runtime proof subsume the iteration bound, or layer on top?

## Cross-references
`proof_caching.md` (makes the proof burden affordable); `verified_gated_ml_optimizer.md` (the LLM-authoring loop); Cathedral `scheduler_and_resources.md` (totality-on-handlers + WCET consume this); Omega effects (chapter 18), contracts / proof obligations (chapters 7, 9).
