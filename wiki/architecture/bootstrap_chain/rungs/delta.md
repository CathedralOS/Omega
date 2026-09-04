# Rung: Delta

[Chain overview](../bootstrap_chain.md) | Prev: [Gamma](gamma.md) | Next: [Epsilon](epsilon.md)

Delta is the small typed pure functional language above Gamma. It adds
static nominal algebraic data, exhaustive matching, checked signed integers,
immutable bytes, monomorphic functions, mutual recursion, and proper tail
calls. It exists to implement the Epsilon compiler.

The normative contract is
[`source/delta/LANGUAGE.md`](../../../../source/delta/LANGUAGE.md). The canonical
compiler must be written in Gamma and emit canonical Gamma source. The selected
Beta-authored Gamma evaluator executes that compiler over Delta source and can
execute the resulting canonical Gamma receipt. The selected 1,280-line
in-progress source enforces Delta's textual-ASCII byte envelope, identifier and
reserved-name grammar, signed-literal range, and exact global function
resolution/arity. It rejects duplicate parameters and uses generated Gamma
names outside Delta's identifier alphabet, so lowering cannot capture authored
locals. It lowers
arbitrary-field recursive algebraic data and
declaration-order exhaustive `match`, while a whole-program pass enforces
global declaration order, nonempty data, unique namespaces, and exactly one
`main`. The admitted complete edge remains absent while normative `Bytes`, full
checking, checked arithmetic, proper-tail lowering, and profile closure remain
open.
