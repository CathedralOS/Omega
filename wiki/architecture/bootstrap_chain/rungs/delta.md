# Rung: Delta

[Chain overview](../bootstrap_chain.md) | Prev: [Gamma](gamma.md) | Next: [Epsilon](epsilon.md)

Delta is the small typed pure functional language above Gamma. It adds
static nominal algebraic data, exhaustive matching, checked signed integers,
immutable bytes, monomorphic functions, mutual recursion, and proper tail
calls. It exists to implement the Epsilon evaluator.

The normative contract is
[`bootstrap/delta/LANGUAGE.md`](../../../../bootstrap/delta/LANGUAGE.md). The canonical
compiler must be written in Gamma and emit canonical Gamma source. The selected
Beta-authored Gamma evaluator executes that compiler over Delta source and can
execute the resulting canonical Gamma receipt. The selected 2,236-line source
is one canonical request entry plus 14 manifested shared implementation members.
It enforces Delta's textual-ASCII byte envelope, identifier and
reserved-name grammar, signed-literal range, and exact global function
signatures. A two-pass immutable metadata catalog preserves forward and mutual
nominal declarations without repeated whole-source lookup. Function metadata
likewise retains ordered resolved signatures and typed parameter environments
once for all calls. Sparse bytewise tries store only present child edges. Its
immutable exact-name environments reject unknown locals and
duplicate active parameter, `let`, or pattern binders while preserving reuse
across disjoint scopes. The same pass checks the scalar/nominal expression,
constructor, pattern, call, arm, and result type relation. Generated Gamma
names live outside Delta's identifier alphabet, so lowering cannot capture
authored locals. It lowers
arbitrary-field recursive algebraic data and exact arbitrary-order exhaustive
`match`, while a whole-program pass enforces
global declaration order, nonempty data, unique namespaces, and exactly one
`main`. That whole-program type check completes before the first output byte;
emission consumes the checked result without repeating declaration and binder
validation, and malformed frontend inputs currently publish nothing. Tail calls remain in
tail position through emitted `if`, `let`, and
lowered `match`; a 100,000-node construction/traversal witness completes in the
selected evaluator's bounded call context. Authored signed arithmetic evaluates
operands once and traps at every Delta overflow boundary. The five typed
`Bytes` builtins use a private length-bearing rope, including checked logical
concatenation and proper-tail lookup. Canonical DCREQ framing and
`ConformanceBytesV1` are executable. Request admission publishes exact DCOUT
frames for malformed framing, unknown profiles, and source-length refusal;
raw-source diagnostics have a separate source-owned entry sharing the same
implementation. The admitted complete edge remains absent while frontend/schema
and later resource/internal DCOUT outcomes, deterministic failure selection,
and final edge closure remain open.
