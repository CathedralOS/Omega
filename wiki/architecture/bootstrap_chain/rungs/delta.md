# Rung: Delta

[Chain overview](../bootstrap_chain.md) | Prev: [Gamma](gamma.md) | Next: [Epsilon](epsilon.md)

Delta is the small typed pure functional language above Gamma. It adds
static nominal algebraic data, exhaustive matching, checked signed integers,
immutable bytes, monomorphic functions, mutual recursion, and proper tail
calls. It exists to implement the Epsilon compiler.

The normative contract is
[`source/delta/LANGUAGE.md`](../../../../source/delta/LANGUAGE.md). The canonical
compiler must be written in Gamma and emit canonical Gamma source. The selected
Gamma and Beta compilers compose that receipt into Alpha. The full source and
tape are currently absent; scalar elaboration experiments pass, while algebraic
data, `match`, `Bytes`, and profile closure remain open.
