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

Gamma's source semantics remain independently fixed by its language contract
and Beta-written reference implementation. The required compiler artifact is
also written in Beta and emits Alpha tape for arbitrary accepted Gamma source.
An interpreter may serve as an early correctness route, but the canonical edge
must yield a standalone tape for the Gamma-written Delta compiler without an
external Beta compiler or host transformation.

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

`source/gamma/interp.beta` and `typeck.beta` are genuine Beta-written Gamma
semantics and useful implementation material. They do not yet constitute the
standalone Gamma-to-Alpha compiler required by D11. The former Beta-written
Delta-to-Gamma route was outside Gamma ownership and is deleted rather than
retained as the Delta edge or a compatibility layer.

## Must not contain

No mutable host memory, hardware boundary, package manager, product optimizer,
or Delta parser hidden in Beta. Proof checking is not a Gamma language feature;
Gamma merely hosts one independent implementation of the Alpha-owned kernel.

## Implementation frontiers

- define the exact Gamma compiler source/artifact closure in Beta;
- reuse the interpreter/type checker as specifications or components without
  turning runtime interpretation into a permanent historical dependency;
- emit exact Alpha tapes and checked source-to-tape certificates; and
- escalate on terrible compiler performance, Alpha verbosity, or proof
  explosion rather than adding special Gamma accelerators.
