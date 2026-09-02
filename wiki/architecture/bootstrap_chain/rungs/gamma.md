# Rung: Gamma

[Chain overview](../bootstrap_chain.md) | Prev: [Beta](beta.md) | Next: [Delta](delta.md)

Gamma is a bounded concatenative compiler machine. Its evaluator is written in
trusted Beta and compiled to Alpha tape by the admitted Beta compiler.

Gamma keeps 64-bit words, an explicit data stack, fixed cells, sealed input,
append-only output, named words, ordinary calls, and explicit tail `jump` and
`branch`. It excludes local variables, heap values, algebraic data, pattern
matching, closures, computed jumps, exceptions, packages, concurrency, and
ambient effects.

Its exact contract is
[`source/gamma/LANGUAGE.md`](../../../../source/gamma/LANGUAGE.md). Its customers
are the Delta compiler and explicitly justified small bootstrap tools. The
Beta-authored evaluator covers the core in a 28-case focused gate. An 81-line
Gamma compiler emits and runs an exact 35-byte addressed-CFG customer.
The evaluator's derived tape admission and complete conformance closure remain
open.
