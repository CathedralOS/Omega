# Terminal Psi operation vocabulary

> **Needs porting.** This document has not been consolidated or vetted for the
> current documentation structure. See the [migration index](../../README.md).

[Documentation index](../../../README.md) | [Pipeline](pipeline.md)

The [portable-product contract](../../../spec/terminal-psi/product.md),
[mathematical proof contracts](../../../spec/proofs/contracts.md),
[boundary-call and realization contract](../../../spec/terminal-psi/boundary_calls.md),
the [immutable byte-view vocabulary](../../../spec/terminal-psi/byte_views.md), and
[observations](../../../spec/terminal-psi/observations.md),
[encoding](../../../spec/terminal-psi/encoding.md),
[verification](../../../spec/terminal-psi/verification.md),
[calls and outcomes](../../../spec/terminal-psi/calls_and_outcomes.md),
[control flow and ranking](../../../spec/terminal-psi/control_flow.md),
[structural access and stores](../../../spec/terminal-psi/structural_access.md),
[loan resources and compatibility](../../../spec/terminal-psi/loans.md),
[structural claims and cleanup](../../../spec/terminal-psi/ownership.md),
[dynamic dispatch](../../../spec/terminal-psi/dynamic_dispatch.md),
[private callbacks](../../../spec/build/private_callbacks.md),
[mathematical proof values](../../../spec/terminal-psi/mathematical_values.md),
[integer certificates](../../../spec/terminal-psi/integer_certificates.md), and
[logical work](../../../spec/resources/logical_work.md),
[content custody](../../../spec/resources/content_custody.md),
[placed access](../../../spec/resources/placed_access.md), and
[component publication](../../../spec/build/component_publication.md) have moved to their
specification owners. This remaining reference carries operation-specific
meaning and validation details until their consolidation;
it does not redefine those migrated subjects.

Implementation entry maps live beside
[Terminal production](../../../../omega-rust/psi/compiler/terminal-production/README.md)
and [Terminal-to-abstract lowering](../../../../omega-rust/omega/pipeline/terminal-psi-to-abstract-operations/README.md).
The [execution board](../../../../TASKS.md) owns unfinished implementation.
Current pre-release producers and consumers move together; stale artifacts reject.

## Terminal requirements

Terminal Psi is immutable and self-contained. It contains no arena handle that
requires `TypedTrees`, source syntax, the producer compiler, or instruction
selection to interpret its meaning. It contains:

- concrete machines and instantiated types;
- explicit typed blocks, block parameters, values, calls, transitions,
  continuations, and terminals;
- lowered predicates over the same stable value/place identities as execution;
- typed structural places, including ordinary and provider-backed roots plus
  field, dynamic-index, dereference, and range/subextent projection;
- explicit cleanup, transfer, conservation, invalidation, and boundary actions
  on edges, plus suspension plans on the exact incomplete calls they govern;
- closed semantic operation variants, including scoped CPU/device ordering
  events; and
- fingerprinted contracts, obligation schemas, authorized admission sites,
  trust attribution, and work identities.

The first quotient correspondence carrier is proof-only. `TerminalModule`
retains a strictly identity-ordered table for the narrow monomorphic, total,
direct faithful `define` certificate and the position-preserving direct
`lift` certificate backed by explicit `Congruence` and
`ForwardPreconditionTransport` evidence. The transport payload retains every
public-`Q`, representative-`P`, and congruence-legality fact's Left/Right
application side, authored source coordinate, and selected-theorem coordinate.
The codec serializes the complete
source-free certificate, including its canonically role-ordered theorem
evidence, and rederives its retained identity on decode. The role discriminant
precedes the selected application and role-specific payload in identity and
canonical bytes. Representation validation rejects missing, duplicate,
reversed, surplus, role/payload-mismatched, and unknown-tag evidence before it
independently reconstructs the theorem, correspondence, eligibility,
fact-major/source and theorem-coordinate order, exact congruence-`P`/transport-
`P` join, and direct-result shape. Format 53 / vocabulary 56 carry this
strengthened source-free contract. A nonempty table is still
rejected by execution validation, owns no machine or operation, and
does not authorize a representative call. The explicit producer attachment is
therefore a canonical-retention prerequisite, not executable quotient
lowering. A separate proof-only package-review row now covers the total, direct
`define` correspondence and the position-preserving direct transport-backed
`lift` by transactionally rederiving the complete source batch and retaining
the selected package's exact public callable, theorem/contract-fact
coordinates, relations, eligibility, and result coordinate. Package-review
schema 120 / row schema 78 / recovery schema 16 already encode the closed
transport kind and complete two-role payload, so no schema bump is required.
It is not ordinary checked package projection: quotient contract calls and
executable requests remain blanket-rejected, while two-argument lift, adapted,
literal, permuted, repeated, generic, private, and broader forms plus the full
package-review migration remain open.

Omega task activation applies the same authority split after checking.
`TaskRuntime::{start,try_start}` retains its compact specialization value only
as a report coordinate; provider planning derives a domain-separated strong
commitment over the exact checked TaskRuntime requirement and operation, exact
package-qualified target/entry signature including parameter modes, and target
machine-contract commitment. The task runtime receipt binding carries both
values but derives invocation identity from the strong commitment alone, so
compact equality never authorizes a different specialization.

## Structural predicates

A nonempty path to a relevant Boolean field of a record parameter retains every
canonical structural-field identity and rebases across structural Unit calls.
For a field- or literal-fixed-index-projected structural argument, the caller's
canonical argument path is prepended to the callee's parameter-relative Boolean
path. Canonical predicate segments distinguish verifier-owned field identities
from exact array indices. The verifier independently traverses both declared
record and fixed-array paths, requires in-bounds indices, structural
intermediates, and a Boolean leaf, and rejects absent, erased, truncated,
mistyped, out-of-bounds, or redirected paths. Built-in Boolean equality,
inequality, negation, and conjunction may compose multiple relevant member
paths and literals; every nested path is independently traversed and rebased.
Equality, inequality, and ordered comparisons also accept same-typed relevant
fixed-integer member paths; terminal terms retain both the canonical path and
the exact integer type, and the verifier checks that annotation against the
declared leaf. Built-in fixed-integer `&`, `|`, `^`, and `~` compose the same
typed member terms without an arithmetic proof obligation; overloaded forms and
the distinct address carrier remain outside this bounded structural slice.
Checked production applies that address fence both to direct member predicates
and to whole-record leaf expansion; the source contract may remain in checked
identity, but Terminal lowering receives no portable scalar term and rejects it.
Whole-root and all-field-projected structural calls reconstruct those predicates
across the callee boundary by prepending the caller's canonical argument path to
every callee-relative integer-member path, including operands nested beneath
bitwise terms. The verifier repeats that substitution independently and rejects
a redirected continuation even when the redirected path reaches another valid
same-typed leaf. A built-in Boolean
disjunction retains two distinct canonically ordered proposition branches; each
branch may contain the same accepted Boolean or integer-member predicate forms.
Production and independent verification recursively rebase every branch across
whole-root and all-field-projected calls. Both codecs retain the proposition;
the semantic codec rejects nested, duplicate, or noncanonically ordered
disjunction rows. Whole-record equality does not add an opaque aggregate term:
for two same-typed `Equatable` parameters, checked production retains the
language-defined inline field expansion. A finite nonempty tree containing only
relevant Boolean, fixed-integer, IEEE `f32`/`f64`, and supported byte-sequence
leaves becomes one flat canonical conjunction. Float leaves use an atomic
format-annotated IEEE `==`
proposition rather than mathematical `Equal`, preserving NaN non-reflexivity and
signed-zero equality. Direct float-field `!=` uses the same atomic proposition
with an explicit comparison kind, preserving the complementary IEEE result
without a second verifier family. Whole-record float `!=` reuses the canonical
equality conjunction as the premise of `P -> Falsehood`, so aggregate float
negation adds neither a duplicate leaf family nor De Morgan permutations. Each
leaf keeps its left and right parameter root;
call verification independently substitutes both roots and rejects redirecting
either operand even when the replacement path is otherwise valid and
same-typed; float leaves additionally require the exact declared format.
Byte-sequence leaves use a separate atomic content-equality proposition over
two nonempty structural paths. Terminal structural identity distinguishes a
borrowed view from bounded owned storage and retains the bounded carrier's exact
capacity, but does not expose a native pointer/length descriptor. Equality is
defined only by equal live lengths and equal live byte prefixes: pointer
identity, capacity, and bytes beyond the live length are irrelevant. The
verifier independently requires both resolved leaves to have byte-sequence
carrier types, and call substitution rebases both roots. The bounded slice
admits field-to-field whole-record equality for `&[u8] in Domain` and
`[u8; N] in Domain`; text literals and direct text `!=` remain fenced. The
current semantic codec, proof-bundle codec, and installation record encode this
vocabulary. A genuinely zero-member record instead normalizes equality to the
existing Boolean `true` term; inequality uses the existing negation, and calls,
codecs, verification, fixed fuel, and interpretation reuse that carrier. An
all-erased record is not empty and remains fenced. Payload-less sums retain
their closed case roster as exact Terminal structural case identities. Equality
is the canonical flat conjunction of both case-membership implications for
each case; inequality is that complete equality proposition implying
falsehood. The verifier resolves each subject and case independently.
Payload-bearing pure sums additionally retain each exact case-payload field.
For direct relevant Boolean, fixed-integer, IEEE, and byte-sequence payload
leaves, equality is a canonical disjunction whose arm for each case conjoins
membership of both roots in that case with the exact payload-leaf equalities;
inequality is that complete disjunction implying falsehood. A case path uses an
exact case identity followed by its exact payload-field identity, and the
verifier and codecs reject unknown or redirected identities. One relevant
acyclic record or pure-sum tree directly held by a case-payload field also
expands its supported leaves transitively. Those paths retain every exact
alternating case, payload field, enclosing record field, and leaf identity in
order through nested sums, and whole-root calls independently rebase both
operands. Direct whole-root mixed shapes retain both common fields and a closed
case roster. Their equality is one canonical conjunction: supported
common-field leaf equalities in declaration order followed by one
source-ordered disjunction whose arms contain matching membership for both
roots and the selected case's supported payload-leaf equalities. Inequality is
that complete equality proposition implying falsehood. Whole-root Unit calls
independently rebase both operands, while codec format 33 / vocabulary 35,
verifier, fixed-fuel, interpreter, and installation format 40 preserve and
replay the exact common-field, case, and payload-field identities. One bounded
nested form is also included: a whole-root acyclic record may contain exactly
one relevant direct field whose type is the existing mixed shape. Every mixed
common-field, case-membership, and payload-leaf path retains that enclosing
field as its first segment, and whole-root Unit-call rebasing preserves it on
both operands. No new proposition or format is added; independent structural-
path replay rejects a substituted enclosing field. The next bounded form
admits exactly two enclosing relevant record fields. Every path carries both
field identities before the sole mixed
occurrence; equality, inequality, whole-root Unit-call rebasing, codecs,
verification, fixed fuel, and interpretation replay that exact ordered chain,
and mutation of either field rejects independently. The following bounded form
admits exactly three enclosing relevant record fields. Equality, inequality,
whole-root Unit-call rebasing, codecs, verification, fixed fuel, and
interpretation retain all three exact field identities before the sole mixed
occurrence, and mutation of any prefix rejects independently. A fourth bounded
form admits exactly four enclosing relevant record fields and replays the same
complete ordered path through equality, inequality, whole-root call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A fifth bounded form admits exactly five enclosing relevant record
fields and replays the same complete ordered path. A sixth bounded form admits
exactly six enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A seventh bounded form admits exactly seven enclosing relevant record
fields and replays the same complete ordered path through whole-root equality,
inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. An eighth bounded form admits
exactly eight enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A ninth bounded form admits exactly nine enclosing relevant record
fields and replays the same complete ordered path through whole-root equality,
inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. A tenth bounded form admits
exactly ten enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. An eleventh bounded form admits exactly eleven enclosing relevant
record fields and replays the same complete ordered path through whole-root
equality, inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. A twelfth bounded form admits
exactly twelve enclosing relevant record fields and replays the same complete
ordered path through whole-root equality, inequality, Unit-call rebasing,
codecs, verification, fixed fuel, interpretation, and independent prefix
mutation. A thirteenth bounded form admits exactly thirteen enclosing relevant
record fields and replays the same complete ordered path through whole-root
equality, inequality, Unit-call rebasing, codecs, verification, fixed fuel,
interpretation, and independent prefix mutation. A fourteenth bounded form
admits exactly fourteen enclosing relevant record fields and replays the same
complete ordered path through whole-root equality, inequality, Unit-call
rebasing, codecs, verification, fixed fuel, interpretation, and independent
prefix mutation. Fifteen or more enclosing fields, mixed values below case
payloads or another mixed shape, two mixed sibling fields, direct projected
mixed comparisons, recursive cycles, address and erased payload equality,
written `equals` bodies, and runtime sum layout remain outside this bounded
terminal slice. When an acyclic relevant record field reaches a payload-bearing
sum, the same sum proposition is retained below that field path, and independent
verification preserves the complete `Field -> Case -> Field` identity chain.
Direct source-call rebasing
through a sum-bearing projection remains fenced with runtime sum projection and
cleanup.
Arithmetic over
same-typed relevant fixed-integer members accepts Exact addition, subtraction,
and multiplication: each member or fixed-integer-literal operand retains its
exact checked carrier, nested operations remain typed `ExactIntegerAdd`,
`ExactIntegerSubtract`, or `ExactIntegerMultiply` terms, and whole-root or
all-field-projected calls rebase every member leaf recursively. The verifier
independently repeats that substitution and validates every declared leaf and
arithmetic-node type; both codecs preserve the nested term. Policy-selected
fixed-integer members also accept the total Wrapping and Saturating forms of
addition, subtraction, and multiplication. The terminal term retains the exact
selected behavior, and projected calls, codecs, verification, fixed fuel, and
interpretation preserve it without an overflow obligation. Wrapping left and
right shifts are likewise retained as total structural terms: the value's
carrier and the independently typed integer count remain distinct, and the
language-defined Euclidean count reduction survives projected calls, codecs,
verification, fixed fuel, and interpretation without a count obligation. Exact
right shifts accept a self-proving in-range literal count or a complete retained
package proving a runtime count nonnegative and below the shifted carrier width.
Exact left shifts require the same count evidence plus carrier-tight value bounds
at the greatest possible count; a zero count or a compile-known value that shifts
safely is self-proving. The producer canonically orders the complete requirement
package, and projected calls rebase one exact obligation per requirement.
Independent verification reconstructs the count and overflow checks and rejects
missing or weakened evidence. Direct Trapping arithmetic remains forbidden in
predicate terms. An explicit fixed-integer or address `embed` instead lowers to
an unbounded proof-`Int` term carrying the source carrier identity and exact
derived range; an explicit same-carrier `as` lowers to Exact arithmetic and
retains its discharged representability obligations. Wrapping and Saturating
predicate nodes retain their distinct total denotations. Terminal Psi never
creates a proof-side Trapping node or a predicate-generated crash effect.
Executable Trapping operations separately retain their compiler-owned
primitive trap predicate and path-conditioned crash site. Verification checks
that denotation against the primitive catalog and proves the derived guard is
covered by the authored same-cause route disjunction.
Exact division and remainder accept a
same-carrier literal divisor only when it is nonzero and cannot trigger signed
`MIN / -1` overflow. Wrapping and Saturating division and remainder accept any
same-carrier nonzero literal, including signed `-1`: their selected policy
defines the `MIN / -1` result, while division by zero remains illegal.
A whole-root structural Unit closure may instead name a runtime integer-member
divisor. For Exact operations, each machine's complete bounded `requires`
package must prove one of the verifier-owned totality shapes: `1 <= divisor`,
`divisor <= -2`, or the joint signed bounds `divisor <= -1` and
`MIN + 1 <= dividend`. For Wrapping or Saturating operations the corresponding
package need only prove the divisor nonzero through `1 <= divisor`,
`divisor <= -2`, or `divisor <= -1`. Checked plans retain those packages without
source handles and terminal Psi publishes the exact requirements. Every direct
or all-field-projected structural call carries one exact obligation per callee
requirement; the producer rebases the target place through the caller's
canonical field prefix, cites the matching caller assumption, and emits a
replaceable certificate. Independent verification reconstructs that prefix,
repeats the rebasing, and checks the assumption index before codec or
interpretation. Removing evidence or weakening or redirecting a bound rejects.
Case-payload paths and imported crash capsules remain fail-closed.
Structural/content contracts reject because custody effects require their own
vertical slice rather than an ordinary scalar flag.

The interpreter uses owned call frames and charges the call before entering the
callee. Sponsor exhaustion in the callee resumes without replaying that paid
call. A callee crash escapes as the original no-successor crash site and uses
that callee edge's fuel charge; call composition records the surviving route
without fabricating or double-charging another executable crash. Validation
rejects recursive call graphs until terminal Psi can carry and verify the
required tail-position and ranking evidence. Fixed-fuel derivation includes
separate acyclic callee return/crash bounds: caller tails compose only with
normal returns, while callee crash paths terminate at their own edge. It retains
its own cycle rejection as defense in depth.

Omega selects each callee's native calling plan and evaluates arguments into
disjoint frame spills before filling their ABI homes. Assignment retains
explicit register or outgoing-stack destinations. Emission materializes the
complete outgoing area, including Microsoft x64 shadow space, preserves x86
call alignment and the AArch64 link register, and emits a typed internal-call
relocation tied to the exact Psi operation and callee. Conditional-control
emission preserves live entry registers across condition calls and rebases
relocations from independently encoded conditions and arms into final function
order.

An unconditional crash continuation requires no caller-side machine-code
branch: the verified internal call reaches the emitted callee crash leaf and
cannot return along that execution. Omega still resolves the typed call
relocation and preserves the callee leaf; it does not reinterpret a crash as a
scalar result.
