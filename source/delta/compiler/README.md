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
census, complete D31 structural type formation, a source-backed resolution
catalog, ordered local-value resolution, scalar/aggregate value-place facts,
and pure final symbolic-Alpha encoder.
It validates every source byte
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
qualified-only receiver forms, states, and exact nonempty whole-program
exhaustion. D36's declaration syntax and cross-kind callable census are now
implemented. Direct-qualified, unqualified, named-data receiver, and exact
sealed-boundary applications now retain source identity and settled result
facts; continuation/state classification and the remaining body/control
judgments stay with the rest of D37's premise-DAG composition.
D38's source-backed `.as_slice` receiver/result facts and separate extra-call
rejection for the resulting array view are implemented; their lowering and
executable controls remain.
AST-to-symbolic-Alpha lowering, `main`, and final publication remain
implementation gaps. Q4 blocks promotion of the incomplete entry-diagnostic
judgment. D31's profile-independent structural type-formation judgment is now
implemented; its
physical storage realization remains later than complete checking, with D34
now fixing its over-`Int` demand representation. The existing source is
therefore not yet a compiler edge and no validation may describe it as one.

D22 fixes the collection phase's namespaces and ordinary local rules. D24
completes the collector contract for transition-arm binders and exact
same-phase `InvalidBoundary`/`DuplicateName` ordering. The implemented census
first collects every owner row and exact qualified machine identity from source
spans, then scans member, parameter, state, let, and transition-binder scopes.
It compares authored bytes exactly, keeps local identities source-shaped,
collects transition binders independently of later case and arity validity,
and returns the globally earliest declaration-start failure. Its D36 extension
compares every case against every qualified machine admitted beneath the same
unique data owner, ignores fields and arity, and anchors a collision at the
later callable declaration. Any duplicate owner row is ambiguous, including
same-kind duplication, so it contributes no inferred owner kind and admits no
machine to a data-owner callable registry until repaired. The current source
type-checks through the Gamma frontend; no census behavior is claimed as
executed while the canonical Gamma compiler edge is incomplete.

Type formation walks every authored type after that census with an explicit
stored/parameter/local/return/nested placement. It derives array-length,
mixed-data, standalone-value `u8`, misplaced-`never`, forbidden-view, sealed-
`Console`, unknown-name, and recursive-value candidates independently,
suppressing the complete child subtree of a forbidden view. Stored `u8` and
`u8` nested beneath an array or view remain valid; a standalone parameter,
local, or return does not. One candidate merge chooses only by packed source
coordinate and treats a distinct same-anchor reason as an internal
contradiction. Accepted programs retain explicit source-ordered record/sum rows
so `data X {}` is concretely a zero-field record, plus all direct data-
containment edges. Recursion checks each edge with a visited-owner graph walk,
marking every edge in a value cycle at its named-reference coordinate without
expanding every path through a shared acyclic graph. The winning candidate is
now promoted after successful census. Remaining expression typing and the
body/control judgments are the next semantic phase. Entry facts may be retained
alongside them, but Q4 must total
their reasons, anchors, and ties before the shared final-phase candidate is
promoted.

The resolution catalog retains one row per formed top-level declaration and
keeps members, cases, bodies, and states inside their original AST owner. It
classifies qualified machine owners without numeric node IDs, provides exact
owner/machine/member/state lookups, and compares types structurally using
nominal name equality and semantic array lengths. Constructor and qualified-
machine lookup remain structurally distinct after D36's census rejects any
shared owner/name callable spelling. An unqualified receiver is a syntax
rejection at its `&` rather than an ownerless catalog row. The retained bare-
versus-parenthesized qualified-expression distinction now feeds an owner-aware
direct qualified-callable lookup. An unresolved qualified machine declaration
owner contributes `UnknownName` at that owner spelling even when unused. A
neutral minimum-coordinate bucket deduplicates one reason/coordinate and
retains distinct ties for the eventual D37 internal-contradiction promotion;
traversal and DCOUT order never choose between them.

Ordered local resolution then walks every expression-bearing entry, state, and
transition position. Machine parameters remain active across the invocation;
state parameters and lets remain body-local; entry locals never leak into a
state; and transition binders exist only in their own continuation. Each body
precollects its pending lets without granting visibility, so a current or later
let reference contributes `UseBeforeInitialization` while a genuinely absent
value name contributes `UnknownName`. Successful references retain their exact
parameter, let statement, or pattern/binder declaration and are keyed by
expression constructor plus exact span; a postfix node therefore cannot borrow
the fact for its same-start base. Call heads, discarded postfix statements, and
transition continuations now carry distinct expression-use facts rather than
one overloaded non-value bit. This is durable identity custody for typing and
lowering, not a partial acceptance judgment.

The same expression walk now retains complete settled result facts
without introducing a second recursive checker. Integer, character, and
Boolean literals are `i32`; strings are immutable `&[u8]`; and resolved
parameter/local reads retain both their value type and their assignable storage
type. Reading `u8` storage will yield `i32` while preserving a `u8` place for
the later store check. Groups preserve the complete value/place result;
negation and every binary operator consume complete `i32` operands and produce
a non-place `i32`. Their resultless/`never` branches are retained independently
at D37's enclosing-relation or exact-call anchors, and a missing sibling never
manufactures an operator mismatch or result.

Direct qualified constructors and data machines retain exact callable custody
before context, arity, or argument typing. The same generalized ledger retains
unqualified machine applications in ordinary, nested-call-head, and postfix-
statement positions, including arbitrarily grouped bare-name heads.
Constructor and machine lookups remain independent, and the semantic machine
lookup admits only a catalog row already resolved beneath a data owner.
Authored arguments are walked as siblings; arity depends only on admitted
identity and count; and the type join waits for every required value fact. A
compatible constructor yields its nominal non-place value, while a compatible
receiverless machine with authored parentheses yields its declared value,
resultless, or `never` fact.
Stored constructor payload `u8` accepts an `i32` value and leaves range
enforcement to runtime `ByteRange`. A bare resolved machine in an ordinary
value position contributes D36's `TypeMismatch` at the qualified expression
start. Unqualified machine lookup precedes local-value fallback because call
grammar selects that namespace; a genuinely absent head retains the local
`UnknownName`/`UseBeforeInitialization` premise, while a completed noncallable
local is `TypeMismatch` at the application. One admitted-signature join owns
arity, complete argument typing, and value/resultless/`never` production for
direct-qualified, unqualified, data-receiver, and sealed-boundary spellings.
Exact catalog ownership gives `self` its nominal value and storage place only
inside a receiver-bearing qualified data machine and all of that machine's
states; invalid `self` remains factless pending Q10's rejection ruling.
Grouped named-receiver heads preserve exact data-machine or boundary-member
custody before receiver-place and signature checks. Static machines are never
inferred to have a receiver. Same-spelled record fields fall back to ordinary
field custody and the general value-call failure, while call syntax selects a
same-spelled machine and an invalid case/machine collision selects neither.
Chained resultless/`never` receivers retain their category/terminal failures.
Grouped bare-qualified call heads now transfer machine custody to the exact
outer application; a completed constructor or machine application followed by
another suffix remains an ordinary noncallable base. Discarded postfix
statements carry their outer anchor through grouping and select an unqualified
machine before local fallback. Only exact machine custody with authored
application syntax is category-admitted. Constructors, bare machines, known
qualified fields/boundary members, and complete noncallable values fail before
arity/type checking; an admitted machine with a missing result gains no
dependent mismatch. Value/resultless machine results may be discarded, while
a successful `never` result remains input to the later block-flow judgment.
The bare head and outer application intentionally retain distinct exact-AST
callable rows; lowering must query the exact application row and must not treat
every ledger row as an executable call.
Full continuation/state admission remains with the wider body/control judgment.

Named-record projection retains both the exact owner declaration and authored
field for later ordinal/layout recovery. A field inherits a place only from a
place-valued base, and storage `u8` reads as value `i32` without losing its
`u8` place. Array indexes follow the same rule; immutable-view indexes never
produce a place. Array/view `.len` is non-place `i32`, and complete range
slices produce a non-place immutable view. Present indexes and bounds require
complete `i32` facts, but bounds remain runtime `Bounds` checks rather than
static folding. A field selector in call-head position is classified against
its complete receiver before ordinary field fallback. Record fields literally
named `len` or `as_slice` remain ordinary fields because contextual array/view
members are selected only after base-type classification.

The same selector classifier owns settled negative projection: an absent
ordinary member is `UnknownName` at its spelling; a known case, machine, or
boundary member used as a value selector is `TypeMismatch`; contextual names
on unsupported complete receivers are `TypeMismatch`; and resultless/`never`
bases retain category/terminal failures. Grouped call heads reuse the same
classifier before the separate call suffix. Index and slice joins retain each
available resultless/`never` child failure independently, but derive
unsupported-base and non-`i32` relations only when every required operand has
a value fact. Missing siblings therefore manufacture no projection relation or
parent result. Static index/slice checking still imposes no range judgment.

The fact pass follows D37 by producing no parent fact or dependent diagnostic
while a consumed child premise is absent. Admitted callable arity is a sibling
judgment and can therefore coexist with an independently failing
argument, while result typing waits for complete compatible values. A separate
call suffix on a complete ordinary value or resultless result is `TypeMismatch`;
an embedded `never` result is `InvalidTerminal`. Every authored argument keeps
its explicitly anchored `never` branch independently of callee admission and
arity; Q11 blocks the unspecified resultless-argument anchor. Let and `assert`
relations consume only complete values. Assignment checks its left value/place
and right value branches independently, and compares against the retained
storage type only after both facts exist; this admits `i32` establishment into
`u8` storage without treating its zero-extended read type as the place type.
Remaining transition/control context, return, and terminal-flow candidates
still require the rest of the D37 premise DAG. The fact pass now
implements D38's accepted receiver/result, exact contextual failures, and
`array.as_slice()` value-call rejection; D37 still fixes how the remaining
nested failure candidates compose. This foundation does not claim final
acceptance or runtime realization.

## Contract-derived conformance plan

This is the compact case matrix for the eventual adjacent executable gate. It
derives from D17, D22, D24, D36, D37, D38, and `LANGUAGE.md`; it is not an unrun
corpus and records no execution evidence. Cases become executable only through
the real Gamma-written compiler and its selected D19 adapter.

| Area | Positive controls | Negative controls and exact obligation |
| --- | --- | --- |
| Source and lexical phase | all permitted ASCII/trivia; every keyword/operator boundary; decoded character and string escapes | each of the six lexical reasons; first invalid byte/opening token; a lexical failure wins over every parse or later-phase defect |
| Syntax | every type, expression, statement, terminal, transition, boundary/data/machine/state form; comments between tokens; exact nonempty EOF | `UnexpectedToken` at the offending token, including `&` where an unqualified machine parameter must begin, and `UnexpectedEnd` at source extent; empty source; missing/trailing delimiters; positive, array-length, and postfix-decorated `2147483648`, while direct unary `-2147483648` parses |
| Declaration census | owner/unqualified-machine spelling reuse; qualified versus unqualified machine distinction; member/local reuse; local reuse across entry, distinct states, and sibling transition arms | boundary/data owner collision; duplicate machine/member/payload/parameter/state/let/transition binder; same-owner case/qualified-machine callable collision regardless of arity; active machine/state/local/binder shadowing; globally earliest declaration-start coordinate across `DuplicateName` and `InvalidBoundary`; ambiguous owner contributes no inferred boundary kind |
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
- every case and qualified machine under one data owner shares the D36 callable
  spelling registry, while fields remain outside that cross-kind check;
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
lengths 1 and `INT32_MAX`, one zero-field record value, stored or nested `u8`,
`never` only as the outer return type, views only as parameter/local roots, and
`Console` only at `Main.console`. Negative controls include zero at its length
literal, mixed data at its declaration name, standalone parameter/local/return
`u8`, every misplaced `never`, every forbidden outer view, and every other
`Console` placement. Nested defects beneath a forbidden view do
not displace its outer `EscapingView`; structurally impossible same-anchor
reason collisions are internal contradictions. D34 storage-profile controls cover
an unused oversized type, nested and disjoint individually excessive arrays,
one reachable decisive array with its length-literal coordinate, and
aggregate-only record/sum/root exhaustion with no coordinate. Exact demands
remain exact; larger demands use D34's `INT64_MAX` witness. Both storage
refusals require `requested > limit` and publish no tape. Adjacent controls
exercise zero-sized multiplication, exact `INT64_MAX`, and the first larger
demand without taking a Gamma trap.

Resolution-catalog, local-resolution, and expression-fact controls remain
planned, not claimed execution: forward data
owners, unknown qualified owners, same-spelled type and unqualified machine,
distinct qualified/unqualified machines, D36 case/machine collision rejection,
mandatory parentheses on zero-parameter machine calls, state-name reuse across
separate machines, parameter and ordered let visibility,
`UseBeforeInitialization` versus `UnknownName`, entry/state
isolation, arm-local binder visibility, exact same-start postfix separation,
literal/read/group/arithmetic facts, record-field custody, `u8` read/place
splitting, array-place versus immutable-view indexing, every optional slice-
bound shape, `.len`, positive receiver-scoped `self`, grouped named-data and
sealed-`Console` receiver calls, authored-field fallback, same-spelled
field/machine call selection, static-machine-via-receiver refusal, receiver
place refusal, unknown receiver members, and chained resultless/`never`
receiver failures. D38 adds a
place-valued fixed array and computed `array[i()].as_slice` positive case;
view `.as_slice` as `TypeMismatch`; a non-place array result as `InvalidPlace`;
the parsed extra-call form `array.as_slice()` as `TypeMismatch`; an ordinary
record field named `as_slice`; and `f().as_slice = x` producing only the inner
receiver `InvalidPlace` under premise closure. Constructor and
machine rows remain structurally distinct after the census establishes one
callable spelling; no body context, arity, or expected type may select between
colliding declarations. Controls cover both declaration orders, nullary and
payload cases, a same-spelled field/qualified-machine positive case, an
unqualified receiver at its `&`, explicit `()` on a zero-parameter machine, and
a bare machine identity as `TypeMismatch` in an ordinary expression. D37
premise-DAG controls include unresolved callee versus place checking,
wrong arity alongside an independently failing argument, resultless/`never`
value use, projection reason/anchors, and exact let/assignment/assert/return
relations. D38 totals `.as_slice` receiver validity and once-evaluation.

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
