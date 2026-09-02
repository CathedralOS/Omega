# Rung: Gamma

[Chain overview](../bootstrap_chain.md) | Prev: [Beta](beta.md) | Next: [Delta](delta.md)

Gamma is a strict first-order functional S-expression calculus. Its evaluator
is written in trusted Beta and compiled to Alpha tape by the admitted Beta
compiler.

Gamma keeps checked `Int`, immutable `Bytes`, heterogeneous pairs, `if`,
single-binding `let`, unary first-order calls, mutual recursion, bounded
allocation profiles, sealed input, and returned values. It excludes user-defined
algebraic data, pattern matching, arbitrary function arity, a proper-tail
guarantee, mutation, raw memory, closures, higher-order values, macros,
polymorphism, general GC, continuations, exceptions, packages, interactive
evaluation, and ambient effects.

Its exact contract is
[`source/gamma/LANGUAGE.md`](../../../../source/gamma/LANGUAGE.md). Its customers
are the Delta compiler and explicitly justified small bootstrap tools. The
Beta-authored evaluator covers the narrowed core in a 62-case focused gate. A
96-line Gamma compiler emits and runs an exact 35-byte addressed-CFG customer.
The evaluator's derived tape admission and complete conformance closure remain
open.
