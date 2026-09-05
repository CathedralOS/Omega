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
execute the resulting canonical Gamma receipt. The selected 2,606-line source
is one canonical request entry plus 45 manifested shared implementation members.
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
once for all calls. Sparse bytewise tries store only present child edges.
Insertion tail-builds missing suffixes and rebuilds traversed existing edges
from a counted immutable ancestor spine, preserving exact terminal options and
prior roots without identifier-length-dependent Gamma call depth. The extra
ancestor pairs still consume Gamma's finite arena. Its
immutable exact-name environments reject unknown locals and
duplicate active parameter, `let`, or pattern binders while preserving reuse
across disjoint scopes. The same pass checks the scalar/nominal expression,
constructor, pattern, call, arm, and result type relation. Generated Gamma
names live outside Delta's identifier alphabet, so lowering cannot capture
authored locals. It lowers
arbitrary-field recursive algebraic data and exact arbitrary-order exhaustive
`match`, while a whole-program pass enforces
global declaration order, nonempty data, unique namespaces, and exactly one
`main`. That whole-program type check and selected-profile schema validation
precede a complete expanded Gamma lowering plan. Only then does emission print
the plan, without repeating declaration and binder validation or numeric
spelling/range checks. Tail calls remain in
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
local and pattern conflicts (9/10), unknown types and names (11–14), type and
arity disagreement (15/16), duplicate and nonexhaustive matches (17/18),
missing `main` (19), and entry-schema mismatch (20) also have owned DCOUT
publication. Entry schema runs only after the complete frontend succeeds.
Raw-source diagnostics have a separate source-owned entry sharing the same
implementation. Declaration traversal follows authored order; each parameter
conflict precedes its own annotation, parameters precede the result type, and
the whole declaration phase precedes all bodies. Grammar also owns D30's
1,024-level expression `parse_depth` profile: the first level-1,025 expression
returns `Incomplete` code 8 at its source start with limit/requested 1,024/1,025.
This check follows balanced parsing and precedes that expression's grammar
judgment, not node or queue allocation. Body typing consumes retained
expression and pattern nodes and propagates canonical failures. Lowering
consumes retained nodes with explicit visit/resume continuations, retaining
binding identities and expanded expression-list heights in the Gamma plan.
Serialization consumes that plan rather than interpreting Delta constructs.
Stack-safe compiler traversal does not establish generated Gamma admission: each generated
body remains subject to the selected evaluator's separate 255-list nesting
bound. Recording height does not yet normalize or lift over-height bodies.
Other compiler-owned resource/internal outcomes, successful generated
admission throughout Delta's 1,024-level depth profile,
and final edge closure remain open;
underlying evaluator failures do not stand in for those compiler outcomes.
