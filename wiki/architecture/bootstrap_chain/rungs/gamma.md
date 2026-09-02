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
Beta-authored evaluator covers the core in a 29-case focused gate. An 81-line
Gamma compiler emits and runs an exact 35-byte addressed-CFG customer.
A 186-line Gamma reconstructor independently reproduces the evaluator's exact
4,312-byte tape from its addressed Beta source. Tape admission and complete
conformance closure remain open.

The selected 725-line Gamma compiler emits canonical addressed Beta. Its
3,490-line Beta self-receipt assembles to the exact 26,674-byte native compiler
tape, and both evaluator and native executions reproduce that receipt. A
near-limit 336,681-byte Gamma program expands to 2,772,595 Beta bytes and the
same 1,048,547-byte Alpha tape as the retained direct comparator. The adjacent
oversized Alpha candidate rejects before publication in both routes.
