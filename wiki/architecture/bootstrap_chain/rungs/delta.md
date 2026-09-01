# Rung: Delta

[Chain overview](../bootstrap_chain.md) | Prev: [Gamma](gamma.md) | Next: [Epsilon](epsilon.md)

Delta is the small typed pure functional language above Gamma. It adds
static nominal algebraic data, exhaustive matching, checked signed integers,
immutable bytes, monomorphic functions, mutual recursion, and proper tail
calls. It exists to implement the Epsilon compiler.

The normative contract is
[`source/delta/LANGUAGE.md`](../../../../source/delta/LANGUAGE.md). The canonical
compiler must be written in Gamma and emit Alpha tape directly. Its source and
tape are currently absent; the older imperative Gamma language and compiler
remain retired rather than preserved as an alternate route.
