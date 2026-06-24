# Rung: Gamma — safe definitional computation

[Lattice overview](../bootstrap_lattice.md) | Prev: [Beta](beta.md) | Next: [Delta](delta.md)

Gamma is where computation becomes *safe and definitional*: enough to write
parsers, validators, interpreters, and eventually a logical checker in a
substantially safer language than raw assembly. It is the rung the
[Delta](delta.md) proof checker will be written in.

## Adds

- algebraic data types and pattern matching
- pure functions and structural recursion
- a simple static type system
- possibly **total** computation (every function terminates — see honest edge #4
  below)

## Written in

Beta. Gamma's type checker and reference interpreter are beta programs.

## Meaning

A gamma program means what a **gamma reference interpreter written in beta** does
with it. The gamma *compiler* (to alpha/beta) is then an acceleration that must
behave like the interpreter — not the definition.

## Must not contain

No mutable memory, ownership, regions, or effects (those are
[Epsilon](epsilon.md)). No proof objects or logical calculus (that is
[Delta](delta.md)). No contracts, refinement, or dependent types (those are
[Omega](omega.md)). Keeping gamma to pure, safe, definitional computation is what
lets the delta checker written in it be small and auditable.

## Totality (honest edge)

If gamma is total, it cannot contain a plain interpreter for a Turing-complete
language — the interpreter would loop forever on looping input. Reference
interpreters then become **fuel-bounded**:
`interp(program, fuel) -> Result | OutOfFuel`. This is a feature: it makes the
slow reference route's cost bounded and explicit, and it is exactly
[`totality_and_bounded_computation.md`](../../../design_briefs/totality_and_bounded_computation.md).
Decide it deliberately; it reshapes every interpreter in the spine.

## Current repo reality

Two things live here, pulling in opposite directions:

- `gamma.alpha` (~3540 lines) — the **old compiler-first imperative** v13:
  variables `a`–`j` in registers, arithmetic, `if`/`else`, `while`, `print`/`read`.
  Compiler written in alpha-asm, assembled by beta; not self-hosting. Parked.
- `interp.beta` — a **new interpreter-first reference interpreter**, stage 1
  (`test-interp.sh`), written in **Beta** (compiled by the self-hosting `bc`). A
  pure functional, **fuel-bounded** core: integers, top-level recursive functions,
  `if`, `let`, arithmetic/comparisons (`fac 5 → 120`, `fib 10 → 55`, `gcd → 12`).
  This is the architecturally-favored shape — *meaning is the interpreter* — and
  resolves the first divergence below. Stage 2 (DONE) adds the
  gamma-defining features (ADTs + pattern matching), so the Delta checker can now
  be rewritten in it cleanly (its hand-encoded tagged nodes are exactly that pull).

The old gamma diverges from the target in two ways the architecture wants
reconciled — `interp.beta` is the start of that reconciliation:

- **Compiler-first, not interpreter-first** — `gamma.alpha`'s compiler *is* the
  definition; the lattice wants a reference interpreter to define meaning, with
  the compiler checked against it. `interp.beta` is that reference interpreter.
- **Imperative, not functional/total** — `gamma.alpha` has no algebraic data,
  pattern matching, or totality. `interp.beta` is functional + fuel-bounded;
  ADTs + pattern matching landed in stage 2 (`Z`/`S`, `Nil`/`Cons`, `Pair`; `match` with nullary, applied-binding, and catch-all patterns).

The pragmatic question is whether to grow the current gamma toward the target or
to treat the current gamma as a stepping stone and introduce the
interpreter-first functional gamma as its successor.

## Open questions

- Interpreter-first reconciliation (above) — the single biggest fork for the
  current repo trajectory.
- Totality vs partiality, and the fuel discipline for interpreters.
- The minimal type system: just enough to make the delta checker safe to write,
  no more.
