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
execute the resulting canonical Gamma receipt. The selected 2,693-line source
is one canonical request entry plus 23 manifested shared implementation members.
It enforces Delta's textual-ASCII byte envelope, identifier and
reserved-name grammar, signed-literal range, and exact global function
signatures. A complete lexical pass follows source-envelope validation and
precedes retained balanced-tree parsing and structural grammar; numeric spelling
is checked before range. Explicit stacks and counted worklists traverse nested
syntax without recursive Gamma call depth. Balanced parsing completes before
grammar roles, and the retained accepted program reaches declaration collection.
Complete type, constructor, and function identity collection
precedes declaration-type resolution and body checking, so later global
duplicates are not hidden by earlier type defects. Immutable metadata catalogs
preserve forward and mutual declarations without repeated whole-source lookup. Function metadata
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
validation or numeric spelling/range checks. Tail calls remain in
tail position through emitted `if`, `let`, and
lowered `match`; a 100,000-node construction/traversal witness completes in the
selected evaluator's bounded call context. Authored signed arithmetic evaluates
operands once and traps at every Delta overflow boundary. The five typed
`Bytes` builtins use a private length-bearing rope, including checked logical
concatenation and proper-tail lookup. Canonical DCREQ framing and
`ConformanceBytesV1` are executable. Request admission publishes exact DCOUT
frames for malformed framing, unknown profiles, and source-length refusal;
source-byte rejection (3), invalid token spelling and structural grammar (4),
out-of-range integer literal (5), duplicate type/constructor/function
identity (6/7/8),
missing `main` (19), and entry-schema mismatch (20) also have owned DCOUT
publication. Entry schema runs only after the complete frontend succeeds.
Raw-source diagnostics have a separate source-owned entry sharing the same
implementation. Declaration-type, name-resolution, semantic arity, and other
body failures remain
evaluator-owned Gamma status 249 without publication, not DCOUT. Those
remaining frontend paths, later resource/internal outcomes, deterministic
failure selection, and final edge closure remain open.
