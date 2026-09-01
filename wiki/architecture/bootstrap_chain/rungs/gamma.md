# Rung: Gamma

[Chain overview](../bootstrap_chain.md) | Prev: [Beta](beta.md) | Next: [Delta](delta.md)

Gamma is the small typed pure functional language formerly named Delta. It adds
static nominal algebraic data, exhaustive matching, checked signed integers,
immutable bytes, monomorphic functions, mutual recursion, and proper tail
calls. It exists to implement the Delta compiler.

The normative contract is
[`source/gamma/LANGUAGE.md`](../../../../source/gamma/LANGUAGE.md). The canonical
compiler must be written in Beta and emit Alpha tape directly. Its source and
tape are currently absent; the former imperative Gamma language and compiler
were retired rather than preserved as an alternate route.
