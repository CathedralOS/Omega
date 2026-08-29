# Rung: Gamma — safe definitional computation

[Lattice overview](../bootstrap_lattice.md) | Prev: [Beta](beta.md) | Next:
[Delta](delta.md)

Gamma is the first safe definitional rung: enough algebraic data, pattern
matching, typing, and recursion to implement the Delta compiler without making
Beta understand Delta.

## Adds

- algebraic data types and pattern matching;
- pure functions and recursion;
- a small monomorphic static type system; and
- explicit, bounded evaluation resources.

Gamma's intended source semantics must be fixed independently of every
implementation, but OWNER Q4 has not yet selected one executable language from the
current disconnected interpreter and type-checker surfaces. The required
compiler artifact is written in Beta and emits Alpha tape for arbitrary source
accepted by the eventual contract. An interpreter may serve as an early
correctness route, but the canonical edge must yield a standalone tape for the
Gamma-written Delta compiler without an external Beta compiler or host
transformation.

## Direct responsibility

```text
Beta-written Gamma compiler source
  └─ beta_compiler.tape ─▶ gamma_compiler_bytecode.tape

Gamma-written Delta compiler source
  └─ gamma_compiler_bytecode.tape ─▶ delta_compiler_bytecode.tape
```

Gamma implements the Delta compiler. It does not merely provide an evaluator
for a Beta-written translator that already parsed Delta.

## Current migration

`source/gamma/interp.beta` and `typeck.beta` are bounded, disconnected semantic
oracles and useful implementation material. Neither defines Gamma before OWNER Q4,
and together they do not yet constitute the standalone Gamma-to-Alpha compiler
required by D11. The former Beta-written Delta-to-Gamma route was outside Gamma
ownership and is deleted rather than retained as the Delta edge or a
compatibility layer.

## Must not contain

No mutable host memory, hardware boundary, package manager, product optimizer,
or Delta parser hidden in Beta. Proof checking is not a Gamma language feature;
the universal checker remains Alpha-owned and outside the language rung.

## Implementation frontiers

- define the exact Gamma compiler source/artifact closure in Beta;
- reuse the interpreter/type checker as specifications or components without
  turning runtime interpretation into a permanent historical dependency;
- emit exact Alpha tapes and checked source-to-tape certificates; and
- escalate on terrible compiler performance, Alpha verbosity, or proof
  explosion rather than adding special Gamma accelerators.
