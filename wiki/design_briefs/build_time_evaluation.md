# Design Brief: Build-Time Evaluation — Const Evaluation + Trait Generators

Scouted 2026-06-12 (as "comptime"). Status: DIRECTION SETTLED (2026-07-02, Zach) —
**no keyword, and the term `comptime` is retired as a foreign borrowing** (it is
Zig's name for Zig's staged-metaprogramming mechanism, which this is not).
The concept's name is **build-time evaluation**; staging decisions M1-M6 in
TASKS.md remain the open engineering sequence.

## The settled model (2026-07-02)

- **No marker, ever.** Build-time-evaluability is not a declared property: a
  machine with no declared effects HAS none, so "safe to run during
  compilation" is the absence of effects — already structural, already the
  default. A keyword would restate what the effect system carries.
- **Evaluation time is a fact of the POSITION, not the declaration.** A
  position that needs a value during compilation (an array length, a const
  type argument, a `Layout` policy's `plan()` consumed by the deriver)
  evaluates there; the compiler checks every machine reached is effect-free
  and errors at the use site otherwise, rendering the CHAIN from the position
  to the offending effect (shared diagnostic shape with the abort-authority
  chain).
- **The trait signature is the stability contract.** `Layout::plan` is
  declared effect-free in the trait; conformance already requires signature
  agreement, so an implementation growing an effect breaks at ITS declaration
  — exactly where a keyword would have put the error, with zero new surface.
- **Cross-compilation**: build-time evaluation runs on the host but computes
  TARGET facts — inputs arrive target-resolved (see recommendation 4), and the
  reference interpreter's semantics are target-agnostic.
- The one existing compile-time spelling in the grammar stays: `const N: usize`
  type parameters (a POSITION, on a parameter — never a marker on a machine).

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
- First heavyweight client: programmable layouts
  ([`programmable_layouts.md`](programmable_layouts.md)) — the compiler
  invokes `Layout::plan(schema)` at build time and derives codecs,
  projections, and value types from the validated Plan.

## Recommendations

1. **First position: fixed-array lengths** (`[T; N]` where N is an
   effect-free machine call). Lowest coupling, biggest proof leverage
   (lengths drive index facts). Field defaults stage 2; const type params
   stage 3. (The LAYOUTS client needs none of these first: the compiler
   itself invokes `plan()` — a blessed-trait call site, not a general const
   position — so the layouts ladder can start on the interpreter entry point
   alone.)
2. **Purity gate: reuse decision 12** — build-time-evaluable(callee) =
   transitive effects empty AND no &mut/out params. No annotation; the
   position makes it build-time, the effect system makes it legal.
3. **Termination**: NO new rule (maintainer-corrected 2026-06-12; the
   self-recursion framing was Rust-shaped). General recursion does not
   exist in the language — self-calls are tail self-loops and loops carry
   decreases/measures — so build-time-evaluable machines simply inherit the
   existing termination discipline. Fuel at most as a defense-in-depth
   backstop against checker gaps.
4. **Determinism**: emulate TARGET integer widths in the build-time evaluator
   (the interpreter already has signedness/width adjustment — audit and
   reuse). Host-width leakage is a correctness bug; cross-compilation is a
   stage-1 goal.
5. **`Self::fields` exposes names + types only** (stage 1); offsets/case
   reflection deferred. Access spelling: bracket form `self.[field]` —
   syntactically distinguishes the build-time-unrolled access from static
   field access.
6. **equatable.rs is temporary**: stage 2 rewrites `Equatable` as a core
   trait with a generator body and retires the hand-rolled path. One
   mechanism, no special cases.
7. **Failure UX**: compile error at the const site naming the position and
   the failing machine (no silent fallback), with the call chain to the
   offending effect.

## Touches

omega-symbol-resolved-trees (const queries), omega-interpreter (build-time
evaluation entry point + error reporting), resolved→typed lowering
(array-length const pass; generator expansion pass), omega-validation
(witness facts from const results, later).

## Staging

1. Build-time evaluation entry point + effect gate + target-width audit +
   failure diagnostics; the layouts `plan()` call site as the pilot client
   (see programmable_layouts.md) alongside or ahead of array lengths;
   Equatable-via-generator as a hand-wired pilot.
2. Field defaults; general generator expansion framework; Hashable; retire
   equatable.rs.
3. Const type parameters + substitution; decreases-gated recursion;
   const-driven proof witnesses.
