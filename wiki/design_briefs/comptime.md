# Design Brief: Comptime — Const Evaluation + Trait Generators

Scouted 2026-06-12. Status: AWAITING SIGN-OFF (decisions M1-M6 in TASKS.md).

## Current State

- Direction frozen (decision 8 / chapter 13): NO macros, NO #run. Const
  evaluation = effect-free machines in constant positions; trait
  generators = `default machine` bodies using `Self::fields` reflection,
  expanded per conformance, declarer-only, zero effects.
- Landed precedent: Equatable synthesis (equatable.rs +
  structural_equality.rs) — hand-rolled inline expansion at resolved→typed
  lowering. Trait generators would GENERALIZE and eventually replace it.
- The reference interpreter is the candidate engine: effect-free,
  deterministic, differential-oracle-proven; no new evaluation machinery
  needed.
- Const positions today are parse/literal-only — nothing evaluates.
- Decision 12 landed the INFERRED TRANSITIVE EFFECT SURFACE — directly
  reusable as the purity gate.

## Recommendations

1. **First position: fixed-array lengths** (`[T; N]` where N is an
   effect-free machine call). Lowest coupling, biggest proof leverage
   (lengths drive index facts). Field defaults stage 2; const type params
   stage 3.
2. **Purity gate: reuse decision 12** — `is_const_evaluable(callee) =
   transitive effects empty AND no &mut/out params`. No new annotation; the
   position makes it comptime, the effect system makes it legal.
3. **Termination**: NO new rule (maintainer-corrected 2026-06-12; the
   self-recursion framing was Rust-shaped). General recursion does not
   exist in the language — self-calls are tail self-loops and loops carry
   decreases/measures — so const-evaluable machines simply inherit the
   existing termination discipline. Fuel at most as a defense-in-depth
   backstop against checker gaps.
4. **Determinism**: emulate TARGET integer widths in the const evaluator
   (the interpreter already has signedness/width adjustment — audit and
   reuse). Host-width leakage is a correctness bug; cross-compilation is a
   stage-1 goal.
5. **`Self::fields` exposes names + types only** (stage 1); offsets/case
   reflection deferred. Access spelling: bracket form `self.[field]` —
   syntactically distinguishes the comptime-unrolled access from static
   field access.
6. **equatable.rs is temporary**: stage 2 rewrites `Equatable` as a core
   trait with a generator body and retires the hand-rolled path. One
   mechanism, no special cases.
7. **Failure UX**: compile error at the const site naming the position and
   the failing machine (no silent fallback).

## Touches

omega-symbol-resolved-trees (const queries), omega-interpreter (const-eval
entry point + error reporting), resolved→typed lowering (array-length
const pass; generator expansion pass), omega-validation (witness facts from
const results, later).

## Staging

1. Const eval for array lengths + effect gate + target-width audit +
   failure diagnostics; Equatable-via-generator as a hand-wired pilot.
2. Field defaults; general generator expansion framework; Hashable; retire
   equatable.rs.
3. Const type parameters + substitution; decreases-gated recursion;
   const-driven proof witnesses.
