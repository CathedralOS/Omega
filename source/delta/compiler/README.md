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
top-level declaration/program parser, and pure final symbolic-Alpha encoder. It
validates every source byte before
scanning all tokens and literals, returns the exact lexical reason and packed
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
Whole-closure collection, type/control checking, AST-to-symbolic-Alpha lowering,
`main`, and final publication remain implementation gaps. The existing source
is therefore not yet a compiler edge and no validation may describe it as one.

D22 fixes the collection phase's namespace and ordinary local rules. Q9 still
blocks the complete collector on transition-arm binder scope and the exact
ordering/coordinate of `InvalidBoundary` against duplicate-name issues. No
partial collector is retained while those accepted-language rules are open.

## Contract-derived conformance plan

This is the compact case matrix for the eventual adjacent executable gate. It
derives from D17, D22, and `LANGUAGE.md`; it is not an unrun corpus and records
no execution evidence. Cases become executable only through the real
Gamma-written compiler and its selected D19 adapter.

| Area | Positive controls | Negative controls and exact obligation |
| --- | --- | --- |
| Source and lexical phase | all permitted ASCII/trivia; every keyword/operator boundary; decoded character and string escapes | each of the six lexical reasons; first invalid byte/opening token; a lexical failure wins over every parse or later-phase defect |
| Syntax | every type, expression, statement, terminal, transition, boundary/data/machine/state form; comments between tokens; exact nonempty EOF | `UnexpectedToken` at the offending token and `UnexpectedEnd` at source extent; empty source; missing/trailing delimiters; positive, array-length, and postfix-decorated `2147483648`, while direct unary `-2147483648` parses |
| Declaration census | owner/unqualified-machine spelling reuse; qualified versus unqualified machine distinction; member/local reuse; local reuse across entry and distinct states | boundary/data owner collision; duplicate machine/member/payload/parameter/state/let; active machine/state/local shadowing; globally earliest later declaration independent of a later type/body error |
| Type and body checking | forward owners/machines/states; finite records/sums/arrays; views only in admitted positions; complete scalar and sum transitions | every reason from `UnknownType` through `NonexhaustiveSum`, at the first offending type, expression, statement, pattern, or control target; exact `Console`, `Main`, and entry shapes |
| Symbolic Alpha encoding | exact vectors for all 21 instructions; zero/forward/backward labels and aliases; payload at the exact 262,140-byte cap | empty IR, bad register/label, missing/duplicate label, target at payload end/interior, unknown/truncated replay opcode, and the first instruction crossing the cap; no partial tape |

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

Q9 owns the remaining census rows: mutual and outer-environment conflicts for
transition payload binders, sibling-arm reuse, and competition between
`InvalidBoundary` and `DuplicateName`, including a boundary/data-ambiguous owner.

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
- expose pure `main : Bytes -> DeltaCompileOutcome`, with only
  `Complete(Bytes)` and `Reject(DeltaRejectReason, Int)` authored outcomes;
- compile under D19's sealed `DeltaCompilerV1` profile, which checks the exact
  source-owned entry/outcome schema and a total constructor-to-code bijection
  before emission;
- let the generated adapter own `DCOUT`, its profile-owned explicit reason-code
  table, and outer `Incomplete`/`InternalFailure` outcomes;
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
