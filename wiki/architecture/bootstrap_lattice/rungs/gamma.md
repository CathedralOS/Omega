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

`compiler/gamma/` is a small **compiler-first imperative** language (v13):
variables `a`–`j` in registers, arithmetic with precedence, comparisons,
`if`/`else`, `while`, `print`/`read`, `%`, parentheses. Its compiler is written in
alpha-asm (`gamma.alpha`, ~3540 lines), assembled by beta. It is **not yet
self-hosting**.

This diverges from the target gamma in two ways the architecture wants
reconciled:

- **Compiler-first, not interpreter-first** — today the compiler *is* the
  definition; the lattice wants a reference interpreter to define meaning, with
  the compiler checked against it.
- **Imperative, not functional/total** — today's gamma has no algebraic data,
  pattern matching, or totality. The target gamma is the safe definitional layer
  the delta checker is written in.

The pragmatic question is whether to grow the current gamma toward the target or
to treat the current gamma as a stepping stone and introduce the
interpreter-first functional gamma as its successor.

## Open questions

- Interpreter-first reconciliation (above) — the single biggest fork for the
  current repo trajectory.
- Totality vs partiality, and the fuel discipline for interpreters.
- The minimal type system: just enough to make the delta checker safe to write,
  no more.
