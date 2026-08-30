# Delta compiler owner

The canonical compiler owned here accepts Delta, is implemented in Gamma, and
emits platform-independent Alpha tape:

```text
delta_compiler.gamma → delta_compiler_bytecode.tape
```

The source now exists as an incomplete implementation. Its retained milestones
own the exact D17 rejection/outcome sums, complete lexical phase, native syntax
representation, allocation-free syntax-token scanner, complete type and
expression parser, transition-pattern/control parser, body/state parser,
top-level declaration/program parser, complete D22/D24 source-shaped identity
census, complete D31 structural type formation, and pure final symbolic-Alpha
encoder. It validates every source byte
before scanning all tokens and literals, returns the exact lexical reason and packed
offset, and retains no host-generated token ledger. Syntax nodes are recursive
Gamma values with exact source spans rather than byte-rope records or numeric
arena references. The scanner rescans one token at a time only after the
complete lexical phase succeeds. Its token start, code, end, and literal value
are immediate `Int` results: lookahead may repeat bounded scanning work, but it
authors no transient token objects into the generated program's fixed immutable
heap. The ambiguous arm-level `return expression?` uses the same scalar
lookahead to recognize a complete following `pattern ->` prefix. Parser success
wrappers contain only their native AST value and no duplicated cursor or span.
The encoder accepts a closed, nonempty compiler instruction IR rather than
general Alpha assembly. It lays out dense symbolic labels, emits every Alpha
opcode as exact little-endian raw `.tape` payload bytes, constructs an exact
first-crossing oversize candidate for adapter preflight, and independently
replays instruction boundaries and distinct direct targets before returning
bytes. Balanced serialization avoids linear rope depth. Its immutable heap use
still scales with instruction count and unique direct targets; private budget
exhaustion is an honest outer `Incomplete`, and representative `D` inputs must
be profiled once the compiler edge executes. This implementation type-checks
through the current Gamma frontend gate.

It deliberately has no `main`, emitted placeholder, or canonical tape. Every
D17 grammar form now parses, including boundary/data/machine declarations,
receiver forms, states, and exact nonempty whole-program exhaustion.
Remaining body/control checking, AST-to-symbolic-Alpha lowering, `main`, and
final publication remain implementation gaps. Q4 blocks promotion of the
incomplete entry-diagnostic judgment. D31's profile-independent structural
type-formation judgment is now implemented; its physical storage realization
remains later than complete checking, with D34 now fixing its over-`Int` demand
representation. The existing source is therefore not yet a compiler edge and
no validation may describe it as one.

D22 fixes the collection phase's namespaces and ordinary local rules. D24
completes the collector contract for transition-arm binders and exact
same-phase `InvalidBoundary`/`DuplicateName` ordering. The implemented census
first collects every owner row and exact qualified machine identity from source
spans, then scans member, parameter, state, let, and transition-binder scopes.
It compares authored bytes exactly, keeps local identities source-shaped,
collects transition binders independently of later case and arity validity,
and returns the globally earliest declaration-start failure. Any duplicate
owner row is ambiguous, including same-kind duplication, so it contributes no
inferred boundary kind and suppresses `InvalidBoundary` until repaired. The
current source type-checks through the Gamma frontend; no census behavior is
claimed as executed while the canonical Gamma compiler edge is incomplete.

Type formation walks every authored type after that census with an explicit
stored/parameter/local/return/nested placement. It derives array-length,
mixed-data, misplaced-`never`, forbidden-view, sealed-`Console`, unknown-name,
and recursive-value candidates independently, suppressing the complete child
subtree of a forbidden view. One candidate merge chooses only by packed source
coordinate and treats a distinct same-anchor reason as an internal
contradiction. Accepted programs retain explicit source-ordered record/sum rows
so `data X {}` is concretely a zero-field record, plus all direct data-
containment edges. Recursion checks each edge with a visited-owner graph walk,
marking every edge in a value cycle at its named-reference coordinate without
expanding every path through a shared acyclic graph. The winning candidate is
now promoted after successful census. Body/control checking remains the next
semantic phase. Entry facts may be retained alongside it, but Q4 must total
their reasons, anchors, and ties before the shared final-phase candidate is
promoted.

## Contract-derived conformance plan

This is the compact case matrix for the eventual adjacent executable gate. It
derives from D17, D22, D24, and `LANGUAGE.md`; it is not an unrun corpus and records
no execution evidence. Cases become executable only through the real
Gamma-written compiler and its selected D19 adapter.

| Area | Positive controls | Negative controls and exact obligation |
| --- | --- | --- |
| Source and lexical phase | all permitted ASCII/trivia; every keyword/operator boundary; decoded character and string escapes | each of the six lexical reasons; first invalid byte/opening token; a lexical failure wins over every parse or later-phase defect |
| Syntax | every type, expression, statement, terminal, transition, boundary/data/machine/state form; comments between tokens; exact nonempty EOF | `UnexpectedToken` at the offending token and `UnexpectedEnd` at source extent; empty source; missing/trailing delimiters; positive, array-length, and postfix-decorated `2147483648`, while direct unary `-2147483648` parses |
| Declaration census | owner/unqualified-machine spelling reuse; qualified versus unqualified machine distinction; member/local reuse; local reuse across entry, distinct states, and sibling transition arms | boundary/data owner collision; duplicate machine/member/payload/parameter/state/let/transition binder; active machine/state/local/binder shadowing; globally earliest declaration-start coordinate across `DuplicateName` and `InvalidBoundary`; ambiguous owner contributes no inferred boundary kind |
| Type and body checking | forward owners/machines/states; empty and nonempty records; finite sums/arrays; views only in admitted positions; complete scalar and sum transitions | D31 zero-array, mixed-data, misplaced-`never`, escaping-view, and sealed-`Console` cases; every reason from `UnknownType` through `NonexhaustiveSum`, at its exact structural anchor; no reason-table tie-break |
| Symbolic Alpha encoding | exact vectors for all 21 instructions; zero/forward/backward labels and aliases; payload at the exact 1,048,572-byte `AlphaBootstrapV2` cap | empty IR, bad register/label, missing/duplicate label, target at payload end/interior, unknown/truncated replay opcode, and the first instruction crossing the cap; no partial tape |

The payload row describes the current `AlphaBootstrapV2` profile selected by
D23. Its exact cap, depth-20 target trie, replay bounds, oversize candidate, and
adjacent rejection move with the seeds, compilers, generated-memory maps,
checker, outcome tables, and gates. The symbolic encoding and replay rules do
not otherwise change.

D22 rows already settled by the third line include these discriminator pairs:

- a type owner and an unqualified machine may share a spelling, while boundary
  and data owners may not;
- `parse` and `Owner::parse` are distinct machine identities, while two exact
  owner/name pairs conflict;
- fields and cases share their data-owner member scope, but a member and bare
  local may share a spelling;
- machine parameters conflict in the entry and every state body; state
  parameters and lets conflict only in their active body; and
- entry and sibling state bodies may reuse local spellings.

D24 adds separate transition-binder controls: sibling arms may reuse a spelling,
while one arm cannot reference another arm's binder (`UnknownName`); duplicate
binders within one arm and collisions with each active outer-local class are
`DuplicateName`. An unknown case or wrong payload arity does not suppress that
earlier census, so the suite also pins the two-round
`DuplicateName`-then-`UnknownName` result for an unknown case and the
`DuplicateName`-then-`ArityMismatch` result for a known case with the wrong
payload arity. Mixed collection controls cover both source orderings of
unrelated `DuplicateName` and `InvalidBoundary`, plus a boundary/data-ambiguous
owner that produces only its duplicate until repaired.

D31 makes the type-formation gate finite and exact. Positive controls include
lengths 1 and `INT32_MAX`, one zero-field record value, `never` only as the
outer return type, views only as parameter/local roots, and `Console` only at
`Main.console`. Negative controls include zero at its length literal, mixed data
at its declaration name, every misplaced `never`, every forbidden outer view,
and every other `Console` placement. Nested defects beneath a forbidden view do
not displace its outer `EscapingView`; structurally impossible same-anchor
reason collisions are internal contradictions. D34 storage-profile controls cover
an unused oversized type, nested and disjoint individually excessive arrays,
one reachable decisive array with its length-literal coordinate, and
aggregate-only record/sum/root exhaustion with no coordinate. Exact demands
remain exact; larger demands use D34's `INT64_MAX` witness. Both storage
refusals require `requested > limit` and publish no tape. Adjacent controls
exercise zero-sized multiplication, exact `INT64_MAX`, and the first larger
demand without taking a Gamma trap.

Runtime conformance must execute all nine settled traps—`Overflow`,
`DivisionByZero`, `SignedDivisionOverflow`, `ShiftCount`, `ByteRange`, `Bounds`,
`NonBoolean`, `Assertion`, and `NonExhaustiveTransition`—and preserve the exact
stdout prefix before each trap. Resource conformance is parameterized by the
selected compiler/application profile rather than invented constants: for every
source, immutable-heap, recursion/step, emitted-tape, and output bound, exercise
the exact admitted boundary and its adjacent refusal, require outer
`Incomplete`, and prove that no partial compiler artifact is published.

The superseded Beta Delta-to-Gamma route, Darwin-native publication tree, and
restricted Delta-written native compiler prototype are deleted rather than
retained as alternate compiler architecture. The prototype implemented neither
this Gamma-written edge nor full Omega `D`; moving it would have preserved the
wrong identity, while adapting its monolithic restricted frontend and Darwin
backend was less economical than authoring the specified direct components.

## Required replacement

- author `delta_compiler.gamma` against D17 and
  [`../LANGUAGE.md`](../LANGUAGE.md);
- expose pure `main : Bytes -> DeltaCompileOutcome`, with `Complete(Bytes)`,
  `Reject(DeltaRejectReason, Int)`, and D31/D34's attributed/aggregate
  application-static-storage refusal outcomes;
- compile under D19's sealed `DeltaCompilerV1` profile, which checks the exact
  source-owned entry/outcome schema and a total constructor-to-code bijection
  before emission;
- let the generated adapter implement D30's 4-MiB input profile,
  1,048,572-byte output maximum, exact `DCOUT` identity/table, and outer
  `Incomplete`/`InternalFailure` outcomes, including validation of D31/D34's sole
  source-authored `Incomplete` resource;
- compile it with `gamma_compiler_bytecode.tape`;
- emit one exact Alpha tape without external older-rung semantic tools;
- reconstruct Gamma source and Alpha artifact semantics independently;
- check direct source-to-tape refinement and negative mutations; and
- keep any native execution as transparent Alpha-seed packaging or an optional
  checked general Alpha realization.

Any new validation placed here must reconstruct the Gamma-source-to-Alpha-tape
edge for `delta_compiler.gamma`. Generic custody, repeated-execution, or native
publication machinery does not belong here.

The active migration order lives in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## Deletion condition

This implementation owner is retained because its exact path is part of the
canonical lattice contract. Delete any child subtree that does not reconstruct,
implement, or test
`delta_compiler.gamma → delta_compiler_bytecode.tape`; replace the owner only
atomically with a changed, explicitly ruled lattice topology.
