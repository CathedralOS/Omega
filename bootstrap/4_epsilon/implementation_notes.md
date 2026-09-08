# Epsilon evaluator implementation notes

See [the source guide](README.md) for the evaluator entrance, phase ownership,
and ordered source manifest. These notes describe implemented semantics,
diagnostic contracts, validation, and remaining completion requirements.

The canonical evaluator owned here accepts and executes Epsilon and is
implemented in Delta:

```text
Gamma evaluator + Delta-written Epsilon evaluator + exact Epsilon source
  -> Epsilon execution
```

The incomplete implementation retains the exact rejection/outcome sums, complete lexical phase, native syntax
representation, allocation-free syntax-token scanner, complete type and
expression parser, transition-pattern/control parser, body/state parser,
top-level declaration/program parser, complete source-shaped identity
census, complete structural type formation, a source-backed resolution
catalog, ordered local-value resolution, scalar/aggregate value-place facts,
and deterministic body-candidate promotion.
It validates every source byte
before scanning all tokens and literals, returns the exact lexical reason and packed
offset, and retains no host-generated token ledger. Syntax nodes are recursive
Delta values with exact source spans rather than byte-rope records or numeric
arena references. The scanner rescans one token at a time only after the
complete lexical phase succeeds. Its token start, code, end, and literal value
are immediate `Int` results: lookahead may repeat bounded scanning work, but it
authors no transient token objects into the generated program's fixed immutable
heap. The ambiguous arm-level `return expression?` uses the same scalar
lookahead to recognize a complete following `pattern ->` prefix. Parser success
wrappers contain only their native AST value and no duplicated cursor or span.
It deliberately has no final evaluator `main` or composed executable identity.
The staging execution path starts from `Main::main` accepted by the current
checking pipeline; the remaining conformance obligations below still apply.
Local, machine-parameter, state-parameter, and transition-payload bindings retain
exact checked declaration identity while their roots distinguish invocations.
Ordinary values are immutable scalar, record, fixed-array, or sum snapshots; view
values retain a descriptor for their backing and range. Places retain a root
identifier, an ordered field/index path, and the leaf's storage type.
Field paths use the checked owner and member identities, not copied type spans
or source spellings. Same-type sibling fields and different array elements
therefore remain distinct instances.

Record and array snapshots share immutable sparse children. An unwritten child
uses its declared zero home without allocating the full array or record tree.
Record children retain a list of exact checked field identities. Array children
retain their declared extent and an immutable sparse interval tree over indexes.
An empty tree supplies the typed zero; a compressed leaf stores one index/value;
a branch splits its current interval at the canonical midpoint. Reads and updates
validate the visited path, including its saved split and leaf index, not untouched
siblings. Updates reconstruct only that path and share other branches. At most
31 partitions cover every admitted positive `i32` extent. Projection-parent
reconstruction uses a separate stack, never an array or record child list.
Assignment replaces the selected subtree, including all its descendants;
binding or returning an aggregate value does not alias its source root. Root
identifiers are monotonic. Invocation completion removes callee roots without rewinding the
identifier counter or changing surviving caller-owned roots. State transfer
removes old block bindings but retains roots referenced by captured view
arguments, surviving machine bindings, or receiver custody. Later transfers
reconsider retained roots, so an expired view does not keep its backing forever.
This is a staging storage representation, not a complete application storage
realization or a discharge of the physical resource profile.

Grouped/local reads and `assert` use the same scalar evaluator as
`Console.write_byte` and `Console.exit_process` arguments. Argument arithmetic
or bounds traps precede publication, byte-range checks, and process exit. Output
prefixes survive exact exit, arithmetic, shift-count, `ByteRange`, and
`Assertion` trap outcomes. Every scalar operator, including short-circuit
Boolean logic, bitwise operations, shifts, division, and remainder, executes.
Byte reads zero-extend to `i32`; byte stores trap as `ByteRange` outside `0..255`
without committing an update. Assignment selects its complete place once before
evaluating the right side. Indexed stores check bounds before that right side,
then write through the selected path into the post-RHS store. Indexed reads of
places use the post-index store; projections of ordinary values retain their
captured snapshots.

Views retain an ultimate backing and an element range, never another view
variable's home. String literals decode the closed escape set into immutable
byte backing. `.as_slice` retains the selected array place over its exact full
range without copying or establishing new array storage. Ordinary slices of
non-place array values instead share the existing immutable snapshot. Subviews
compose offsets against the same backing. Slice evaluation runs the base, low,
and high expressions in order before checking the complete range; omitted
bounds use zero and the base length. Place-backed view indexes read from the
post-index store. View indexes never expose a place, including for aggregate
elements.
Local view facts likewise carry no place despite their private descriptor home.

All four Console operations have staging execution paths. `read_byte` uses one
sealed input cursor shared by calls and states, returns each byte as `i32`, and
returns stable `-1` at EOF. Root allocation and reclamation preserve that cursor.
`write_line` evaluates its view argument before reading the exact view bytes
and appending LF; argument traps retain the preceding output without emitting
the line. `write_byte` and `exit_process` retain their existing effect ordering.
The diagnostic driver accepts a private source-length/source/stdin frame. A
validated source view references the source window in the existing Delta byte
sequence; only stdin is reconstructed as a balanced byte tree. Source-relative
reads preserve packed coordinates and cannot cross into the header or stdin.
This test transport is
not the final evaluator request or observation profile. Its result has an
explicit private tag: exit carries the full signed `i32` code and stdout;
trap carries the closed trap kind and exact output prefix; rejection carries
the closed reason and exact source offset. Internal failure and malformed
private input have separate tags; the retired staging tag 4 stays unassigned.
This avoids collapsing exits modulo 256 or discarding checking evidence. The
[execution gate](../../tests/epsilon/interpreted-omega-experiment/README.md#private-execution-observations)
documents the exact byte layout. Outer Gamma failures are not converted into
these diagnostic results.

One checked invocation context drives the same statement/block evaluator for
entry and states. Block falloff retains locals, storage, and output before
terminal execution. Scalar transitions consume the checked subject and pattern
ledgers; only the selected continuation executes, with unmatched scalar subjects
trapping as `NonExhaustiveTransition`. State arguments evaluate left-to-right
against the old bindings and capture all values before reclaiming unneeded block
roots and installing new parameter homes. Transfers preserve machine-parameter
roots and receiver identity and resume the target state through a tail call.
Explicit resultless return, state falloff, and supported Console write/exit
continuations complete the current invocation without executing unused states.

Ordinary machine calls use checked callable identities. A receiver call selects
its complete place before evaluating ordinary arguments left-to-right. Each
argument captures its value before the next argument's effects; callee
parameters and locals receive independent roots. Nested receiver calls share
the selected live place, including fields and indexed records, rather than
copying a receiver out and writing it back on return. Unqualified and receiver
machines support recursion, scalar/record/array/sum value returns, resultless
falloff/return, and explicit exit/trap propagation. Ordinary completion releases
callee roots and returns a value without a place; committed effects on caller
storage survive. Expression outcomes carry committed storage and
output through arithmetic, arguments, indexes, right sides, returns, and
transition subjects. Only the outer entry adapter turns ordinary completion
into process exit; recursive `self.main()` retains the existing receiver.

Sum construction enters through `execution/sums/construction.delta` and consumes
the exact checked constructor owner and case, never a receiver-machine lookup.
Arguments evaluate left-to-right and capture immutable payload snapshots in
declaration order. `sums/defaults.delta` normalizes an unwritten named sum only
when its checked owner is available: the first declared case receives typed
zero payloads, with nested records, arrays, and sums remaining lazy. Concrete
sum values retain exact owner/case identity and independent payload values.

`sums/transitions.delta` evaluates the subject once and selects the first
matching complete checked case or wildcard. Missing admitted sum coverage is
an internal contradiction, not the scalar unmatched-subject trap.
`sums/bindings.delta` establishes fresh payload homes only for the selected arm;
lookup uses exact pattern and binder-declaration identity. Mutating a record or
array binder does not mutate the original subject. A view captured from a
binder array retains that independent backing across state transfer through the
ordinary root-liveness rule; unused binder roots are reclaimed normally.

Constructor arguments immediately establish their corresponding payload fields
under the [constructor payload establishment rule](LANGUAGE.md#epsilon-constructor-payload-establishment-order).
A byte payload outside `0..255` traps as `ByteRange`, preserving effects from
preceding arguments and from evaluating the failing argument itself. Later
arguments do not run: their storage mutations, Console output, traps, and exits
cannot precede this failure. Successful aggregate arguments still capture
immutable snapshots before later arguments run.

Impossible checked states produce internal failure, not `Unsupported`: a
non-call control target, a missing ordinary field reference, a view represented
as aggregate storage, or a logical operator sent directly to the nonlogical
scalar helper. Control failures identify the normalized core's start; missing
field facts identify the complete projection's start; malformed aggregate views
identify their type's start. The pure scalar helper has no source coordinate
and uses the existing internal-offset fallback of zero. These failures carry no
buffered stdout into the private observation. Execution outcomes retain no
`Unsupported` constructor or propagation arm: every implemented branch returns
an ordinary value/control outcome, exit, trap, or detected internal failure.

These storage, view, sum, and call operations also do not establish that the
complete Omega D source executes. Every
grammar form now parses, including boundary/data/machine declarations,
qualified-only receiver forms, states, and exact nonempty whole-program
exhaustion. Receiver-only qualified-machine syntax, ordinary named
`self` binding, and disjoint constructor/method selection are implemented.
Unqualified, named-data receiver, and exact
sealed-boundary applications now retain source identity and settled result
facts; explicit state applications and state/machine collision classification
are retained separately. Transition subjects, resolved patterns, typed payload
binders, and sum coverage now have separate exact custody. Grammar-owned
final wildcard and four-stage pattern judgment are implemented: subject
admission precedes semantic identity ownership, duplicate identity precedes
payload arity, and coverage follows completed pattern premises. Bare-state
judgment and resultless-argument anchor are implemented within the language's
premise-DAG composition. Five local block-exit effects, exact after-`never`
delimiters, falloff checks, and machine-continuation categories are implemented;
the remaining body/control judgments stay open.
Source-backed `.as_slice` receiver/result facts and separate extra-call
rejection for the resulting array view are implemented. Its execution route
retains only a place-backed full view, as described above.
Complete execution, the final evaluator `main`, and exact composition with
Omega D remain implementation gaps. The entry-diagnostic subjudgment is implemented. Profile-
independent structural type-formation judgment is now implemented; its
physical storage realization remains later than complete checking, with the
[resource contract](LANGUAGE.md#10-resource-classification) fixing its over-`Int` demand representation. The existing source is
therefore not yet a compiler edge and no validation may describe it as one.

The [closure-checking contract](LANGUAGE.md#4-names-types-and-closure-checking)
owns collection namespaces, ordinary locals, transition-arm binders, and exact
same-phase `InvalidBoundary`/`DuplicateName` ordering. The implemented census
first collects every owner row and exact qualified machine identity from source
spans, then scans member, parameter, state, let, and transition-binder scopes.
It compares authored bytes exactly, keeps local identities source-shaped,
collects transition binders independently of later case and arity validity,
and returns the globally earliest declaration-start failure. Member-pair and
payload census folds retain the earliest candidate in a tail accumulator, so
wide records do not consume one Gamma call context per field. The declaration
fold likewise finishes each declaration's owner, machine, boundary, and contents
checks before advancing; pending minima do not accumulate across declarations.
The local census likewise carries its earliest candidate and active names in a
tail scan, so body width does not retain one recursive frame per local.
The syntax-selected case and receiver-method namespaces are disjoint.
The entry-shape judgment owns duplicate custody for
`Main`, `Console`, `Main.console`, `Console` members, and exact `Main::main`
only when an authored `Main::main` candidate
exists. Every unrelated duplicate remains in the ordinary census. Any ordinary duplicate owner
row is ambiguous, including same-kind duplication, so it contributes no
inferred owner kind and admits no machine to a data-owner callable registry
until repaired.

State-name comparisons, duplicate selection, and state-local scope census use
tail folds. A machine with many states does not retain one pending minimum
operation per state; the earliest candidate and its existing tie behavior stay
explicit in the accumulator. This is required by the current Omega D parser's
786 authored states, not a new Epsilon state-count limit.

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
expanding every path through a shared acyclic graph. Candidate identity is now
exactly its reason and packed coordinate; same-anchor reason equality derives
from the total reason mapping rather than a parallel integer discriminator.
The recursive-edge candidate fold reverses its search spine and accumulates
right-associated minima. Each reachability query still uses the unchanged full
edge graph; edge count no longer retains one pending return per query. This
does not bound the separate depth of a reachability search through owner types.
The member-type candidate fold reverses the member spine and merges each
candidate on the left of the accumulated result. This preserves the original
right-associated conflict behavior with bounded call depth. Declaration and
statement type-formation folds use the same ordering, so an earlier declaration
does not retain a pending merge while a later machine body is checked. State
type formation also reverses only the list spine and preserves exactly
`min(parameters, min(body, suffix))`. Conflict precedence makes reassociation
invalid even when ordinary distinct-coordinate errors would select the same
minimum. State width therefore consumes no pending return frame per state.
The [checking-invariant controls](../../tests/epsilon/checking-invariants/README.md)
pin this association with synthetic overlapping coordinates. They test the
private candidate algebra, not acceptance of an Epsilon source with those spans.
Data-shape selection and catalog unknown-owner selection likewise reverse only
their declaration traversal spine and retain the original merge ordering.
The winning candidate is promoted after successful census. Final type-
formation entry subjudgment runs before that promotion: no authored
`Main::main` owner/name candidate yields only `MissingEntry` at source extent;
once one exists, every malformed or
absent supporting component is `InvalidEntry`. The Console, Main, and entry
declaration scans retain their original forward seen flags in tail folds.
Every candidate inside those scans has reason `InvalidEntry`, so their minimum
is associative and a forward accumulator cannot introduce a conflict. Missing
components are still considered at the end, and defects never short-circuit
later checks. Entry facts do not enter the later body/control candidate carrier.
The retained expression facts now support
the implemented local block-exit pass. Remaining body/control judgments,
application storage realization, and complete execution remain open.

The resolution catalog retains one row per formed top-level declaration and
keeps members, cases, bodies, and states inside their original AST owner. It
classifies qualified machine owners without numeric node IDs, provides exact
owner/machine/member/state lookups, and compares types structurally using
nominal name equality and semantic array lengths. Constructor and receiver-
machine lookup are structurally distinct by syntax. The language permits shared owner/
name spellings and removes direct qualified static-machine selection. An
unqualified receiver remains a syntax rejection at its `&`; every qualified
machine must begin with its receiver, and a receiverless qualified declaration
is rejected at the first nonreceiver token. An unresolved qualified machine
declaration owner contributes `UnknownName` at that owner spelling even when
unused. A
neutral minimum-coordinate bucket deduplicates one reason/coordinate and
retains distinct ties for the eventual internal-contradiction promotion;
traversal and rejection-reason codes never choose between them.

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
execution, not a partial acceptance judgment.

The same expression walk now retains complete settled result facts
without introducing a second recursive checker. Integer, character, and
Boolean literals are `i32`; strings are immutable `&[u8]`; and resolved
parameter/local reads retain their value type and optional storage place.
Assignable values retain their storage type; immutable views carry no place.
Reading `u8` storage yields `i32` while preserving a `u8` place for
the later store check. Groups preserve the complete value/place result;
negation and every binary operator consume complete `i32` operands and produce
a non-place `i32`. Their resultless/`never` branches are retained independently
at enclosing-relation or exact-call anchors, and a missing sibling never
manufactures an operator mismatch or result.

The current source retains direct qualified constructors before context,
arity, or argument typing; static data-machine custody is absent. The
generalized ledger retains
unqualified machine applications in ordinary, nested-call-head, and postfix-
statement positions, including arbitrarily grouped bare-name heads.
Constructor and machine lookups remain independent, and the semantic machine
lookup admits only a catalog row already resolved beneath a data owner.
Authored arguments are walked as siblings; arity depends only on admitted
identity and count; and the type join waits for every required value fact. A
compatible constructor yields its nominal non-place value. Free machines retain
their declared value, resultless, or `never` fact through the unqualified path.
Stored constructor payload `u8` accepts an `i32` value and leaves range
enforcement to runtime `ByteRange`. A bare resolved machine in an ordinary
value position contributes `TypeMismatch` at the qualified expression
start. Unqualified machine lookup precedes local-value fallback because call
grammar selects that namespace; a genuinely absent head retains the local
`UnknownName`/`UseBeforeInitialization` premise, while a completed noncallable
local is `TypeMismatch` at the application. One admitted-signature join owns
arity, complete argument typing, and value/resultless/`never` production for
direct-qualified, unqualified, data-receiver, and sealed-boundary spellings.
The authored `self` token is an ordinary name expression. A receiver-bearing
qualified data machine installs an ordinary named receiver binding with its
owner's nominal value and storage-place type; states inherit that binding, and
an undeclared `self` follows ordinary `UnknownName` resolution.
Grouped named-receiver heads preserve exact data-machine or boundary-member
custody before receiver-place and signature checks. Same-spelled record fields
fall back to ordinary field custody and call syntax selects a receiver machine;
the language likewise permits a same-spelled case because constructor syntax is disjoint.
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
callable rows; execution must query the exact application row and must not treat
every ledger row as an executable call.
Bare state spelling and the remaining transition judgments stay open as
described below.

Explicit transition applications now join the enclosing machine's state
namespace with the global unqualified-machine namespace before arity or
argument typing. A dual match is `InvalidControlTarget` and retains neither
target. A state-only explicit application retains exactly one exact-AST state
row, resolved on incomplete/failed parameter premises and complete only after
the all-value join; state custody is neither callable custody nor an expression
result. Machine-only and qualified/receiver/boundary applications preserve the
existing callable ledger. Grouping normalizes lookup without changing the
authored continuation anchor, and state/local spelling reuse selects the state
only in this exact control syntax. Constructors, bare machine selectors,
static receiver spellings, and complete field/scalar values receive category
failures before dependent checking; same-spelled authored fields remain fields
without call syntax. The transition contract requires authored argument syntax for every state
transfer: a state-only bare spelling contributes `InvalidControlTarget` at the
continuation start without entering the state-application ledger. That branch
is implemented. Machine-continuation exit effects are implemented as
local block facts and require no reachability pass.

An invalid noncallable application has no result fact to propagate through
outer grouping. Transition admission therefore inspects its ungrouped call
shape and the completed base result before deriving `InvalidControlTarget`.
The rejection keeps the authored continuation start, including an outer `(`;
grouping cannot turn a rejected target into an accepted continuation. An
unresolved base still supplies no inferred target failure.

Each complete transition subject is retained once as scalar `i32` or one exact
nominal sum owner after the ordinary result category relation. Pattern
resolution is source-shaped and independent across arms: scalar selectors have
semantic `i32` identity, qualified cases retain exact owner/member custody, and
subject-owner compatibility grants semantic pattern identity before payload
arity gates complete pattern custody. A later repeated admitted identity is
`DuplicatePattern` even if the first occurrence later fails arity. `true` and
`1` are one selector, as are `false` and `0`, independent of leading zeros.
Only a complete case pattern supplies positional payload types and places to
its binder locals; `u8` payloads therefore read as `i32` while retaining
their storage type. Distinct name-resolved, subject-admitted, identity-
owned, and complete facts replace the former broad `Resolved | Complete`
progress. Every continuation remains independently
checked, and each sum transition retains complete, missing, or unresolved
coverage. Transition grammar admits one optional final wildcard; a following
pattern is `UnexpectedToken`, while a redundant final wildcard remains legal.
Missing complete sum coverage is `NonexhaustiveSum` at the subject after every
pattern premise completes. These branches are implemented.

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
classifier before the separate call suffix. Binary, index, and slice relations
require complete nonterminal results from every operand, including each present
slice bound, before a resultless child can produce a parent `TypeMismatch`.
An unresolved or `never` sibling cannot satisfy that join. Mispositioned `never`
remains an independent child failure at its exact call head. Unsupported-base
and non-`i32` relations additionally require every consumed value fact. Missing
siblings therefore manufacture no parent relation or result. Static index/slice
checking still imposes no range judgment.

The fact pass follows the closure-checking contract by producing no parent fact or dependent diagnostic
while a consumed child premise is absent. Admitted callable arity is a sibling
judgment and can therefore coexist with an independently failing
argument, while result typing waits for complete compatible values. A separate
call suffix on a complete ordinary value or resultless result is `TypeMismatch`;
an embedded `never` result is `InvalidTerminal`. The contract gives every
direct authored argument its independently anchored category branch regardless of
enclosing-callee admission and arity. The argument contract fixes resultless `TypeMismatch` at
the authored argument expression start, including outer grouping, while grouped `never`
retains its exact call-head anchor. This direct-argument rule does not make
a nested binary/index/slice mismatch independent of that expression's own
complete-premise join. Both rules are implemented. Let and
`assert`
relations consume only complete values. Assignment checks its left value/place
and right value branches independently, and compares against the retained
storage type only after both facts exist; this admits `i32` establishment into
`u8` storage without treating its zero-extended read type as the place type.

The enclosing machine's optional return type now reaches entry, state, and arm
returns. Explicit absence/value relations use exact anchors and category
premises without resolving the expression twice. A source-shaped statement
flow fact retains a successful standalone `never` result, diagnoses its first
following executable construct at the exact terminating delimiter, and still
visits every later authored child. A transition after `never` therefore still
checks its subject expression, patterns, and continuation expressions, but
derives neither transition-subject admission nor sum coverage: those parent
judgments require an open statement sequence. A complete record-valued subject
cannot displace the transition's `InvalidTerminal` with a parent `TypeMismatch`;
an independent unknown child name still contributes its own `UnknownName`.
The five-effect carrier settles local
falloff, explicit returns, machine continuations, `never` calls, and state
transfers without a reachability/fixed-point pass. Remaining transition/control
candidates still require the rest of the premise DAG. The completed body
candidate bucket now promotes no candidate to acceptance, one exact reason to
rejection, and a distinct same-coordinate reason tie to internal failure while
the separate fact-producing entry remains available to diagnostic probes. The
fact pass now
implements accepted receiver/result, exact contextual failures, and
`array.as_slice()` value-call rejection; the closure-checking contract still fixes how the remaining
nested failure candidates compose. This foundation does not claim final
acceptance or runtime realization.

## Contract-derived conformance plan

This is the compact case matrix for the eventual adjacent executable gate. It
derives from [`LANGUAGE.md`](LANGUAGE.md); the full
matrix must execute through the Delta-written evaluator, compiled by the
selected Gamma-authored Delta compiler under `ConformanceBytesV1`. The final
Epsilon evaluator request and observation profile remains open.

The current implementation requires a receiver on every qualified data
machine, allows case/receiver-method spelling reuse, normalizes `self` through
ordinary binding lookup, and rejects a direct static-machine spelling. The
matrix below describes that selected behavior.

| Area | Positive controls | Negative controls and exact obligation |
| --- | --- | --- |
| Source and lexical phase | all permitted ASCII/trivia; every keyword/operator boundary; decoded character and string escapes | each of the six lexical reasons; first invalid byte/opening token; a lexical failure wins over every parse or later-phase defect |
| Syntax | every type, expression, statement, terminal, transition, boundary/data/machine/state form; comments between tokens; exact nonempty EOF; one optional final transition wildcard | `UnexpectedToken` at the offending token, including `&` where an unqualified machine parameter must begin, a pattern after `_`, and negative pattern `-`; `UnexpectedEnd` at source extent; empty source; missing/trailing delimiters; positive, array-length, and postfix-decorated `2147483648`, while direct unary `-2147483648` parses |
| Declaration census | owner/unqualified-machine spelling reuse; qualified versus unqualified machine distinction; case/receiver-method spelling reuse; member/local reuse; local reuse across entry, distinct states, and sibling transition arms | boundary/data owner collision; duplicate exact machine/member/payload/parameter/state/let/transition binder; active machine/state/local/binder shadowing; globally earliest declaration-start coordinate across `DuplicateName` and `InvalidBoundary`; ambiguous owner contributes no inferred boundary kind |
| Type and body checking | forward owners/machines/states; empty and nonempty records; finite sums/arrays; views only in admitted positions; unordered exact `Console` member signatures; complete scalar and sum transitions, including redundant final wildcard | zero-array, mixed-data, misplaced-`never`, escaping-view, and sealed-`Console` cases; absent/malformed/duplicate/competing entry shapes; category/semantic-duplicate/arity/missing-coverage transitions; every reason from `UnknownType` through `NonexhaustiveSum`, at its exact structural anchor; no reason-table tie-break |

Declaration-census rows already settled by the third line include these discriminator pairs:

- a type owner and an unqualified machine may share a spelling, while boundary
  and data owners may not;
- `parse` and receiver method `Owner::parse` are distinct machine identities,
  while two exact machine owner/name pairs conflict;
- a case and receiver method under one data owner may share a spelling because
  constructor and receiver syntax select different namespaces;
- fields and cases share their data-owner member scope, but a member and bare
  local may share a spelling;
- machine parameters conflict in the entry and every state body; state
  parameters and lets conflict only in their active body; and
- entry and sibling state bodies may reuse local spellings.

The census requires separate transition-binder controls: sibling arms may reuse a spelling,
while one arm cannot reference another arm's binder (`UnknownName`); duplicate
binders within one arm and collisions with each active outer-local class are
`DuplicateName`. An unknown case or wrong payload arity does not suppress that
earlier census, so the suite also pins the two-round
`DuplicateName`-then-`UnknownName` result for an unknown case and the
`DuplicateName`-then-`ArityMismatch` result for a known case with the wrong
payload arity. Mixed collection controls cover both source orderings of
unrelated `DuplicateName` and `InvalidBoundary`, plus a boundary/data-ambiguous
owner that produces only its duplicate until repaired.

The language makes the type-formation gate finite and exact. Positive controls include
lengths 1 and `INT32_MAX`, one zero-field record value, stored or nested `u8`,
`never` only as the outer return type, views only as parameter/local roots, and
`Console` only at `Main.console`. Negative controls include zero at its length
literal, mixed data at its declaration name, standalone parameter/local/return
`u8`, every misplaced `never`, every forbidden outer view, and every other
`Console` placement. Nested defects beneath a forbidden view do
not displace its outer `EscapingView`; structurally impossible same-anchor
reason collisions are internal contradictions. Planned storage-profile controls cover
an unused oversized type, nested and disjoint individually excessive arrays,
one reachable decisive array with its length-literal coordinate, and
aggregate-only record/sum/root exhaustion with no coordinate. Exact demands
remain exact; larger demands use `INT64_MAX` witness. Both storage
refusals require `requested > limit` and publish no Epsilon observation. Adjacent controls
exercise zero-sized multiplication, exact `INT64_MAX`, and the first larger
demand without taking a Delta trap.

Transition controls cover a pattern after `_`, a repeated `_`, and an
exhaustive sum with a redundant final wildcard; category-incompatible scalar
and case patterns; duplicate cases before payload arity; and missing sum
coverage at the subject. Scalar identity controls pair `true` with `1`, `false`
with `0`, and decimal spellings with leading zeros. Two-round controls repair a
category, duplicate, or arity failure and then expose the previously suppressed
`NonexhaustiveSum`. Scalar misses remain executable
`NonExhaustiveTransition` traps rather than static rejections.

Resolution-catalog, local-resolution, and expression-fact controls remain
planned as retained executable gates rather than claimed default execution:
forward data
owners, unknown qualified owners, same-spelled type and unqualified machine,
distinct qualified/unqualified machines, case/receiver-method reuse,
mandatory parentheses on zero-parameter machine calls, state-name reuse across
separate machines, parameter and ordered let visibility,
`UseBeforeInitialization` versus `UnknownName`, entry/state
isolation, arm-local binder visibility, exact same-start postfix separation,
literal/read/group/arithmetic facts, record-field custody, `u8` read/place
splitting, array-place versus immutable-view indexing, every optional slice-
bound shape, `.len`, ordinary named receiver binding for reserved `self`, grouped named-data and
sealed-`Console` receiver calls, authored-field fallback, same-spelled
field/machine call selection, direct-static-machine refusal, receiver
place refusal, unknown receiver members, and chained resultless/`never`
receiver failures. Array-view conformance adds a
place-valued fixed array and computed `array[i()].as_slice` positive case;
view `.as_slice` as `TypeMismatch`; a non-place array result as `InvalidPlace`;
the parsed extra-call form `array.as_slice()` as `TypeMismatch`; an ordinary
record field named `as_slice`; and `f().as_slice = x` producing only the inner
receiver `InvalidPlace` under premise closure. Constructor and receiver-machine
rows remain structurally distinct by syntax; no body context, arity, or expected
type selects between namespaces. Controls cover nullary and payload cases,
same-spelled case/receiver-machine and field/receiver-machine positive cases, an
unqualified receiver at its `&`, explicit `()` on a zero-parameter machine, and
a direct receiver-machine spelling that cannot act as a static call. Transition conformance adds
distinct bare zero-parameter state, bare parameterized state, bare machine, and
state/machine-collision controls; all share `InvalidControlTarget` at the
continuation start while only the machine-only case retains callable custody.
Premise-DAG controls include unresolved callee versus place checking,
wrong arity alongside an independently failing argument, resultless/`never`
value use, projection reason/anchors, and exact let/assignment/assert/return
relations. Argument conformance adds valid, wrong-arity, unknown/inadmissible-callee, constructor,
and grouped resultless/`never` argument controls, including the absence of a
distinct-reason coordinate tie. Block-exit conformance adds every local block category/effect,
unused states, closed cycles, resultless/`never`/value continuation calls, and
exact after-`never` delimiter and falloff-brace controls. The language specifies
`.as_slice` receiver validity and once-evaluation.

Runtime conformance must execute all nine settled traps—`Overflow`,
`DivisionByZero`, `SignedDivisionOverflow`, `ShiftCount`, `ByteRange`, `Bounds`,
`NonBoolean`, `Assertion`, and `NonExhaustiveTransition`—and preserve the exact
stdout prefix before each trap. Resource conformance is parameterized by the
selected evaluator profile rather than invented constants: for every source,
storage, recursion/step, and output bound, exercise the exact admitted boundary
and its adjacent refusal and prove that exhaustion publishes no Epsilon
observation.

## Required completion

- complete the evaluator closure selected by `epsilon_compiler.delta.sources` against
  [`LANGUAGE.md`](LANGUAGE.md);
- complete every Epsilon expression, statement, state, trap, and Console
  execution rule without turning a private `Unsupported` result into an
  observation;
- define one exact physical evaluator request and observation profile binding
  evaluator source, Epsilon source, stdin, resources, and maximal execution;
- compile the evaluator through the selected Delta and Gamma route;
- compose it with the exact Epsilon-written Omega D source;
- reconstruct Epsilon source and evaluator semantics independently; and
- check direct `RunEpsilon` refinement and negative mutations.

Any new validation placed here must reconstruct the Delta-written evaluator's
execution of Epsilon. Alpha target emission belongs to Omega D and C, not this
owner.

The active migration order lives in
[`TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).

## Deletion condition

This implementation owner is retained because its exact path is part of the
canonical bootstrap-chain contract. Delete any child subtree that does not reconstruct,
implement, or test
`manifested Delta evaluator closure + Epsilon source → RunEpsilon`; replace the owner only
atomically with a changed, explicitly ruled bootstrap-chain topology.
