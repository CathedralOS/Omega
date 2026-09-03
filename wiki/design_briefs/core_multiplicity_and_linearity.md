# Design Brief: Core Multiplicity And Linearity

Settled 2026-07-18 at the semantic level (frozen decision 21), with the
write-only loan access mode added 2026-08-19.
This brief establishes the core usage discipline independently of dependent
types and content-bearing resource decomposition. Terminal consumption is
derived from ordinary by-value ownership and result flow; it has no separate
annotation to settle.

## Multiplicity is a type property

Omega has three usage multiplicities:

- **Unrestricted** values may be copied and discarded. `[copy]` establishes
  this property.
- **Affine** values may be moved at most once and may be discarded. This is
  the default for owned data.
- **Linear** values must be transferred or consumed exactly once. The source
  spelling is `[linear]` on data/type declarations; it is a checked type
  property, not a trait and not a `lin` qualifier attached to every use.

Multiplicity is orthogonal to borrowing, provenance, representation, and
dependent facts. A value can be linear and fixed-size, affine and
value-indexed, or unrestricted and borrowed. `LinBuf<T, n>` combines two
features; it does not define either one.

Multiplicity belongs to `Type`, not to ambient `Prop`. Proposition proof terms
are unrestricted and copyable. One-shot permission or consumable authority is
therefore an affine or linear Type carrier, possibly with zero runtime layout,
and reuses ordinary moves, borrows, state threading, containers, cancellation,
and retirement. A zero-layout Type value needs no `[erased]` marker merely to
occupy no bytes; its ownership obligation remains ordinary Type custody.

Resource-sensitive mathematics is expressed as an object logic over
user-defined carriers and proposition families. Its resource algebra and
entailment rules may model linear logic without making Omega's metalogical
proof terms linear. This keeps mathematical facts reusable while conserving
systems authority through the same ownership calculus as every other value.

`[linear]` and `[copy]` are mutually exclusive. Structural propagation is
mandatory: a record or live sum payload containing a linear value carries its
obligation; a generic container must declare and preserve the multiplicity of
its parameter rather than silently weakening it.

## Establishment and conservation

Establishing a new linear value creates exactly one obligation. Moves, calls,
returns, receives, and storage operations transfer that obligation; terminal
consumers discharge it. Implicit zero-filling does none of these.

Linearity constrains usage, not bit patterns. Keep three states distinct:

1. storage whose bits have been initialized;
2. a semantic value that has been established in that storage; and
3. a live linear obligation owned by a place.

Raw or implicitly zero-filled storage is not automatically an established
linear value. A particular linear type may gate establishment through its own
validity/default-domain invariant. Conversely, an explicitly constructed
all-zero linear value is legal and owed if all-zero is valid for that type.
There is no universal "zero means consumed" inhabitant and linearity does not
imply a zero-excluding predicate.

Conditional resource state is represented honestly as a sum:

```omega
data TaskState<T> {
    case Empty;
    case Live(task: Task<T>);
}
```

The obligation is path-sensitive and belongs to the `Live` payload. The zero
case is ordinary `Empty`; it is not a forged consumed `Task<T>`.

## Facts and permissions use different algebras

Flow analysis may share one CFG traversal and canonical place identity, but it
must not put propositions and resource obligations in one weakenable catalog.

- The proposition context (Gamma) contains facts. Facts can be duplicated,
  weakened, intersected, and forgotten where logic permits.
- The permission/resource context (Rho) tracks established values and loans.
  Entries carry at least multiplicity (`unrestricted`, `affine`, `linear`),
  loan compatibility (`owned`, `shared`, `exclusive`), permitted
  observation/mutation, and lifetime/provenance.

The exact algebra depends on the entry. Shared loans may duplicate or
reborrow; exclusive loans must not overlap; affine ownership may die through
drop; linear ownership must reconcile exactly. At a control-flow join, facts
join by logical rules while resources join by ownership identity and
path-sensitive conservation. A branch may consume a linear value on both
arms, or transfer it on both arms, but may not consume it on one and silently
lose it on the other.

`&write T` is an exclusive loan with mutation but no observation authority.
It borrows an existing valid `T`; it is not a construction slot and never
denotes vacant storage. `&mut T` may attenuate explicitly to `&write T`, but no
readable reference may be reconstructed from the narrower loan. Exclusivity is
load-bearing for non-disclosure: checked code cannot reach the prior contents
through a second alias while the write-only loan is live.

The access restriction composes transitively through calls. A write-only
operation may use structural metadata, written inputs, and explicitly supplied
proof facts, but may derive nothing by loading the referent. Each write must
also satisfy ordinary displaced-custody and invariant-window rules. An opaque
provider's compliance is admitted unless target isolation enforces it; that
does not widen the authority recorded in the source or artifact contract.
The bounded direct-call subloan may select one literal fixed-array index, or
two, three, or four through a recursively literal fixed array, only when every bound is
static and the ultimate leaf is an unrestricted non-Atomic primitive. Dynamic,
range, fifth-index, and aggregate-leaf projections remain outside that rung.

Post-restoration call evidence now admits one direct exclusive child, one
shared child, or one exact two- or three-member concurrent shared-freeze
cohort. A multi-member form must use every borrow-only alias together in one
empty observation call with one shared parameter per member and mutate the
whole restored parent in the immediately following statement. Checked and
Terminal replay every cohort member independently; four-member, sequential,
projected, reordered, or intervening-call forms remain fail closed.

The first bounded whole-root primitive store accepts a landed integer literal
or Boolean literal and preserves its complete write-only parameter row and
preceding exact-typed scalar use through target-neutral abstract operations and
optimization validation. The current native fixed-integer sublane derives the
primitive referent's exact width/alignment, selects a borrowed-reference ABI on
x86-64 and AArch64, and retains the exact declaration, placement, immediate,
and semantic operation. Its Boolean sibling retains an exact one-byte referent
and preceding Boolean definition through target lowering and independent
physical assignment without an integer surrogate. Machine emission rejoins
those facts and emits an exact one-byte store with a dedicated source record;
the record retains the definition ordinal needed for independent replay.
Fixed-integer physical assignment and machine emission independently replay
their joins before producing the non-observing store bytes. Dedicated custody
rows bind every fact to exact code intervals and bytes, which object
construction replays independently and installation format 73 transports
canonically for integer, Boolean, and raw-bit IEEE float literals. Provider
non-observation authority remains absent.

## Consumers and cleanup

A linear value must reach exactly one authorized terminal consumer on every
exit path. An ordinary owned-receiver machine is the general case. A
type-owned automatic cleanup plan may also be that consumer when its hook is
terminating, infallible, nonblocking, nonsuspending, free of abnormal outcomes,
and needs no runtime authority beyond the receiver. Consumption is derived from
ordinary ownership flow: the compiler begins consuming before lending
`&mut self` to the hook. It is never inferred from an authored call to a
borrowed receiver.
`move self` is the ordinary transfer into a consuming machine; no terminal
annotation is added. A returned outcome that contains the same obligation
transfers it back, while an outcome that does not contain it requires the
callee to have consumed it.
Terminal Psi now represents the first such transfer directly for a root-only,
whole-parameter result: the structural result signature and `ReturnStructural`
edge carry one exact live linear value and its ordered whole-root claim set.
The verifier performs the transfer only on that exit; content equality is not
an entry axiom. Checked source produces the exact one-state passthrough, with an
optional finite tail of unqualified, claim-free affine parameters discarded in
reverse order after result materialization. Native realization covers that
exact root form and retains claim identity, register/stack parameter homes, and
cleanup as typed metadata. One final direct `CallStructural` over that same
single-fragment whole root also reaches native code with exact returned-claim
custody. Projections, additional/non-immediate structural calls, and wider
values remain later slices.
Cancellation and failure paths obey the same conservation law. A `try_*`
operation that has not completed must therefore return the live linear value
in its pending/failure case.

Affine ownership permits automatic cleanup by default. Linear ownership permits
it only when the type owner has declared that exact cleanup plan as a valid
terminal disposition. This does not silently turn a fallible or coordinating
protocol into cleanup. A `Task<T>` remains linear without an automatic terminal
disposition: `finish` consumes its lifecycle claim, while moving it into another
owner transfers the obligation. `request_cancel` retains the claim because a
request does not prove that the activation stopped. Scope exit with a live
`Task<T>` is a compile error, not an implicit blocking finish or detach. Strict
result use does not prove this rule: it catches a discarded return, not a bound
handle reaching scope end.

The receiver's type graph retains this distinction directly: `&self` and
`&mut self` are reference types, while bare `self` is owned. Permission-event
discovery therefore treats method-form and static-form consuming calls alike
without guessing ownership from the method name.

Likewise, automatic drop guarantees only the owner-authored terminal
disposition. It does not promise that buffered bytes reached durable storage.
Fallible or suspending work is an explicit `flush`, `close`, `commit`,
`finish`, or cancellation/settlement machine with an ordinary result contract.
If that consumer may suspend or block, its call uses the ordinary `suspend` or
`block` acknowledgement; acknowledgement does not itself discharge the
obligation.

## Relationship to dependent types and resources

Core linearity requires no value-indexed types. The core covers whole-value moves,
linear parameters and returns, structural propagation, path reconciliation,
and explicit consumers. Transactions, acknowledgements, DMA submissions, and
task lifecycle claims are immediate clients.

Divisibility is not implied by linearity. File handles, acknowledgement tokens,
and DMA completions have no content composition operator: the whole-claim
frontier already accounts for each exactly once. A content-bearing exact
qualification may additionally publish one owner-unique conformance to the
core `Content<A>` requirement and project into a compiler-owned partial
composition algebra. The initial closed algebras are canonical disjoint
interval sets and counted quantities over proof-level natural arithmetic.
Fixed-width integer and address inputs reach those algebras by first embedding
uniformly into proof `Int`, then converting exactly under their derived
nonnegativity facts; `Nat` is the content coordinate, not the universal machine
integer denotation.
Ordinary linearity never
implies a content projection.

Owned decomposition proves one n-ary theorem: entry content plus sealed
introductions equals the separated composition of all produced content plus
any content that left checked custody. “Retired” in frontier reports names that
custody exit, not destruction or reclamation. Per-output containment and scalar
measures cannot prove this because individually plausible children may overlap.
Split and merge are the same equation in opposite dataflow directions.

This refines rather than duplicates the edge-cleanup witness. Edge cleanup
accounts for every incoming whole claim exactly once; a content-bearing
transformation additionally accounts for every symbolic unit inside the mapped
claims exactly once. Proof/debug artifacts report both levels in one nested
conservation witness.

Borrowed division remains cheaper. Layout fields, subrange loans, placed views,
and allocator-private geometry retain one owned root and therefore do not split
its content. An allocator that returns an independently owned subextent does
split content and must conserve it. Runtime-indexed owned extraction remains
rejected until the frontier and prover can name the unique moved element.

## Representation law

Multiplicity and establishment are semantic information and must survive
through checked control flow. The IR needs:

- a first-class multiplicity enum, with affine as the default;
- established/unestablished place state distinct from initialized bits;
- a permission context carrying multiplicity, access, and provenance;
- path-sensitive resource state for sums; and
- explicit create, transfer, consume, and affine-drop events.

One canonical permission ledger survives the semantic pipeline with
multiplicity, access, transfer-stable provenance, and carry policy kept as
independent fields. Shared and exclusive loans enter that same context without
being redefined as linear obligations. Every canonical event has one
fail-closed backend realization: exact selected instructions or a narrow
checked no-code reason. Empty, missing, foreign, or invalid realizations publish
no partial ledger.

Permission debt is path-indexed through transparent records, literal-length
arrays, active sum cases, and generic substitutions. A nominal `[linear]` root
owns the obligation; transparent child paths do not create duplicate roots.
Partial moves preserve sibling debt, claim identity, provenance, and effective
carry policy, while duplicate moves and runtime-indexed owned extraction reject.
Unique structural output maps preserve multiple claims through calls,
transitions, and wrappers; ambiguous or bodyless targets fail closed.

Terminal Psi carries the first literal-array instance of that rule: a nonempty
fixed array of linear structural elements has one canonical claim per index.
The checked plan requires the complete dense set; one literal element may pass
to a bodyless boundary or an exact one-parameter ordinary Unit callee, and
verification and interpretation preserve every unselected sibling. Nested or
dynamic indexes, wider signatures, contracts over the projected parameter, and
content-bearing partitions stay outside this bounded carrier.

Platform-entry writes, dispatch/state-call arguments, and synthesized
continuations obey the same event and provenance rules. Affine exits run in
reverse declaration order. The nominal list slice realizes that rule for a
finite nonempty list of whole affine Unit parameters with bounded attached
drops. The ordered actions may share one cleanup target because custody remains
place-specific; Psi charges and executes every invocation while Omega preserves
the list as one artifact action stream. Every action may have a bounded executable
body and may share its cleanup target or helpers; each emitted cleanup call
retains its exact edge/action ordinal. One
root-only structural-result slice admits a
finite consecutive prefix of immutable, unqualified empty-record affine locals:
checked facts retain each dense declaration ordinal and type identity, terminal
Psi explicitly establishes them in declaration order, and return cleanup orders
them in reverse before any affine parameter. This does not generalize local
cleanup to control edges, nominal drop, or partial values. The flat
partial-value slice separately admits a finite source-ordered set of pairwise
prefix-disjoint, nonempty all-field moves from one claim-free affine record and
preserves every maximal live residual subtree in recursive reverse declaration
order through terminal Psi and Omega artifacts.

The literal-array affine slice is separate from that record cleanup rule. For
an owned, unqualified, claim-free `[T; 2]` whose element is an affine structural
record without nominal cleanup, one projected ordinary Unit call may leave the
opposite element as its exact no-code residual. A checked/Terminal successor may
instead move indices `0` and `1` exactly once each, in either authored order,
and then return with no cleanup action. Its custody frontier closes only after
the complete set `{0, 1}` has moved, and interpreter fuel charges both calls,
both callee returns, and the caller return. Because no live residual exists,
this successor establishes no array cleanup ordering rule. Target, machine,
object/image, and installation custody independently rederive the complete
two-call set and preserve either authored order without treating either call
alone as permission to discard the root.

The next exact residual-bearing rung accepts `[T; 3]` under the same ownership,
qualification, claim, element-shape, and nominal-cleanup restrictions. Exactly
two distinct literal indices move through two projected Unit calls in authored
order; the complement is one typed no-code residual on the caller return. The
frontier, interpreter, target and machine lowering, and object/image and
installation replay independently reconstruct that singleton complement,
canonical length-three layout, element stride, offsets, and five closure fuel
units. The implemented carrier still rejects one move because it would require
two ordered residuals; widening that carrier is now engineering work under the
general rule below. Three moves belong to a separate no-residual rung. Because
the shipped rung has only one live residual, it did not itself establish an
array cleanup ordering rule.

The general order is nevertheless fixed. Literal fixed arrays establish
elements in increasing index order and clean the statically known live residual
set in decreasing index order, recursively and with discharged indices absent.
This is the indexed form of reverse establishment: authored projected moves keep
their authored order, while compiler-generated disposal follows the language's
structural order. An ordinary edge abandoning partial construction cleans the
established prefix in reverse; trap and nuclear-abort edges clean nothing. No
runtime liveness bitmap or data-dependent cleanup loop is introduced.

The bounded recursively nested carrier now admits inner lengths three, four,
five, six, seven, eight, nine, ten, eleven, twelve, thirteen, fourteen, and
fifteen under one exact rule. A claim-free, unqualified affine
`[[T; N]; 2]`, where `N` is one of those lengths and `T` is a checked record
without nominal cleanup, may
move exactly one literal leaf from each outer element through two ordinary Unit
calls. Authored calls retain their order; the return cleans every remaining
leaf with outer indices decreasing and then inner indices decreasing. The
length-ten successor carries eighteen no-code residuals and the length-eleven
successor carries twenty, the length-twelve successor carries twenty-two, and
the length-thirteen successor carries twenty-four, the length-fourteen
successor carries twenty-six, and the length-fifteen successor carries
twenty-eight, while retaining the same five call/return fuel units. Inner
length sixteen, another outer length,
same-outer or nonliteral moves, and runtime liveness machinery remain fenced.

The first construction-prefix implementation is deliberately narrower than
that general rule. An uninitialized mutable `[T; 3]`, where `T` is an
unqualified, claim-free empty affine record with no nominal cleanup, may
establish literal indices `0` and `1` in authored order and then reach an
ordinary Unit return. Checked custody represents the two established elements
as zero-ABI affine locals that retain their common array-root type and static
indices. Terminal and every native artifact layer reconstruct establishments
`[0, 1]`, cleanup `[1, 0]`, and two operation plus one return-edge fuel units.
Missing, duplicate, reordered, wrong-root, wrong-length, third, initialized,
nonempty, dynamic-index, qualified, claimed, and nominal-cleanup forms remain
outside this exact engineering rung.

The exact wider successor admits `[T; 4]` under the same restrictions and
establishes literal indices `0`, `1`, then `2`. Ordinary abandonment cleans the
three zero-ABI element occurrences in order `[2, 1, 0]`; Terminal and every
native artifact layer independently retain the common length-four root and
three operation plus one return-edge fuel units. Missing or reordered
establishments, root/index/length drift, cleanup-order drift, and wider prefixes
at that rung remain fail closed without runtime liveness state or a cleanup loop.

The following bounded successor admits `[T; 5]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, then `3`. Ordinary abandonment cleans
the four zero-ABI element occurrences in order `[3, 2, 1, 0]`; Terminal and
every native artifact layer independently retain the common length-five root
and four operation plus one return-edge fuel units. Missing or reordered
establishments, root/index/length drift, cleanup-order drift, and wider prefixes
remain fail closed without runtime liveness state or a cleanup loop.

The next bounded successor admits `[T; 6]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, then `4`. Ordinary abandonment
cleans the five zero-ABI element occurrences in order `[4, 3, 2, 1, 0]`;
Terminal and every native artifact layer independently retain the common
length-six root and five operation plus one return-edge fuel units. Missing or
reordered establishments, root/index/length drift, cleanup-order drift, and
wider prefixes at that rung remain fail closed without runtime liveness state
or a cleanup loop.

The following bounded successor admits `[T; 7]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, then `5`. Ordinary
abandonment cleans the six zero-ABI element occurrences in order
`[5, 4, 3, 2, 1, 0]`; Terminal and every native artifact layer independently
retain the common length-seven root and six operation plus one return-edge fuel
units. Missing or reordered establishments, root/index/length drift,
cleanup-order drift, and other prefix drift remain fail closed without runtime
liveness state or a cleanup loop.

The next bounded successor admits `[T; 8]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, then `6`. Ordinary
abandonment cleans the seven zero-ABI element occurrences in order
`[6, 5, 4, 3, 2, 1, 0]`; Terminal and every native artifact layer independently
retain the common length-eight root and seven operation plus one return-edge
fuel units. Missing or reordered establishments, root/index/length drift,
cleanup-order drift, and other prefix drift remain fail closed without runtime
liveness state or a cleanup loop.

The following bounded successor admits `[T; 9]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, then `7`.
Ordinary abandonment cleans the eight zero-ABI element occurrences in order
`[7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and every native artifact layer
independently retain the common length-nine root and eight operation plus one
return-edge fuel units. Missing or reordered establishments,
root/index/length drift, cleanup-order drift, and other prefix drift remain fail
closed without runtime liveness state or a cleanup loop.

The next bounded successor admits `[T; 10]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, then `8`.
Ordinary abandonment cleans the nine zero-ABI element occurrences in order
`[8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and every native artifact layer
independently retain the common length-ten root and nine operation plus one
return-edge fuel units. Missing or reordered establishments,
root/index/length drift, cleanup-order drift, and other prefix drift remain fail
closed without runtime liveness state or a cleanup loop.

The following bounded successor admits `[T; 11]` under the same restrictions
and establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`,
then `9`. Ordinary abandonment cleans the ten zero-ABI element occurrences in
order `[9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and every native artifact
layer independently retain the common length-eleven root and ten operation plus
one return-edge fuel units. Missing or reordered establishments,
root/index/length drift, cleanup-order drift, and other prefix drift remain fail
closed without runtime liveness state or a cleanup loop.

The next bounded successor admits `[T; 12]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`, `9`,
then `10`. Ordinary abandonment cleans the eleven zero-ABI element occurrences
in order `[10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and every native
artifact layer independently retain the common length-twelve root and eleven
operation plus one return-edge fuel units. Missing or reordered
establishments, root/index/length drift, cleanup-order drift, and other prefix
drift remain fail closed without runtime liveness state or a cleanup loop.

The following bounded successor admits `[T; 13]` under the same restrictions
and establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`,
`9`, `10`, then `11`. Ordinary abandonment cleans the twelve zero-ABI element
occurrences in order `[11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and
every native artifact layer independently retain the common length-thirteen
root and twelve operation plus one return-edge fuel units. Missing or reordered
establishments, root/index/length drift, cleanup-order drift, and other prefix
drift remain fail closed without runtime liveness state or a cleanup loop.

The next bounded successor admits `[T; 14]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`, `9`,
`10`, `11`, then `12`. Ordinary abandonment cleans the thirteen zero-ABI
element occurrences in order `[12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`;
Terminal and every native artifact layer independently retain the common
length-fourteen root and thirteen operation plus one return-edge fuel units.
Missing or reordered establishments, root/index/length drift, cleanup-order
drift, and other prefix drift remain fail closed without runtime
liveness state or a cleanup loop.

The following bounded successor admits `[T; 15]` under the same restrictions
and establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`,
`9`, `10`, `11`, `12`, then `13`. Ordinary abandonment cleans the fourteen
zero-ABI element occurrences in order
`[13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and every native
artifact layer independently retain the common length-fifteen root and
fourteen operation plus one return-edge fuel units. Missing or reordered
establishments, root/index/length drift, cleanup-order drift, and other prefix
drift remain fail closed without runtime liveness state or a
cleanup loop.

The next bounded successor admits `[T; 16]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`, `9`,
`10`, `11`, `12`, `13`, then `14`. Ordinary abandonment cleans the fifteen
zero-ABI element occurrences in order
`[14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and every native
artifact layer independently retain the common length-sixteen root and
fifteen operation plus one return-edge fuel units. Missing or reordered
establishments, root/index/length drift, cleanup-order drift, and other prefix
drift remain fail closed without runtime liveness state or a
cleanup loop.

The following bounded successor admits `[T; 17]` under the same restrictions
and establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`,
`9`, `10`, `11`, `12`, `13`, `14`, then `15`. Ordinary abandonment cleans the
sixteen zero-ABI element occurrences in order
`[15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and every
native artifact layer independently retain the common length-seventeen root
and sixteen operation plus one return-edge fuel units. Missing or reordered
establishments, root/index/length drift, cleanup-order drift, and other prefix
drift remain fail closed without runtime liveness state or a cleanup loop.

The next bounded successor admits `[T; 18]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`, `9`,
`10`, `11`, `12`, `13`, `14`, `15`, then `16`. Ordinary abandonment cleans the
seventeen zero-ABI element occurrences in order
`[16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and
every native artifact layer independently retain the common length-eighteen
root and seventeen operation plus one return-edge fuel units. Missing or
reordered establishments, root/index/length drift, cleanup-order drift, and
other prefix drift remains fail closed without runtime liveness state or a
cleanup loop.

The following bounded successor admits `[T; 19]` under the same restrictions
and establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`,
`9`, `10`, `11`, `12`, `13`, `14`, `15`, `16`, then `17`. Ordinary abandonment
cleans the eighteen zero-ABI element occurrences in order
`[17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal and
every native artifact layer independently retain the common length-nineteen
root and eighteen operation plus one return-edge fuel units. Missing or
reordered establishments, root/index/length drift, cleanup-order drift, and
other prefix drift remains fail closed without runtime liveness state or a
cleanup loop.

The next bounded successor admits `[T; 20]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`, `9`,
`10`, `11`, `12`, `13`, `14`, `15`, `16`, `17`, then `18`. Ordinary abandonment
cleans the nineteen zero-ABI element occurrences in order
`[18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`; Terminal
and every native artifact layer independently retain the common length-twenty
root and nineteen operation plus one return-edge fuel units. Missing or
reordered establishments, root/index/length drift, cleanup-order drift, and
other prefix drift remains fail closed without runtime liveness state or a
cleanup loop.

The following bounded successor admits `[T; 21]` under the same restrictions
and establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`,
`9`, `10`, `11`, `12`, `13`, `14`, `15`, `16`, `17`, `18`, then `19`. Ordinary
abandonment cleans the twenty zero-ABI element occurrences in order
`[19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`;
Terminal and every native artifact layer independently retain the common
length-twenty-one root and twenty operation plus one return-edge fuel units.
Missing or reordered establishments, root/index/length drift, cleanup-order
drift, and other prefix drift remains fail closed without runtime liveness
state or a cleanup loop.

The next bounded successor admits `[T; 22]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`, `9`,
`10`, `11`, `12`, `13`, `14`, `15`, `16`, `17`, `18`, `19`, then `20`.
Ordinary abandonment cleans the twenty-one zero-ABI element occurrences in
order
`[20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`;
Terminal and every native artifact layer independently retain the common
length-twenty-two root and twenty-one operations plus one return-edge fuel
units. Missing or reordered establishments, root/index/length drift,
cleanup-order drift, and other prefix drift remains fail closed without runtime
liveness state or a cleanup loop.

The following bounded successor admits `[T; 23]` under the same restrictions
and establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`,
`9`, `10`, `11`, `12`, `13`, `14`, `15`, `16`, `17`, `18`, `19`, `20`, then
`21`. Ordinary abandonment cleans the twenty-two zero-ABI element occurrences
in order
`[21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`;
Terminal and every native artifact layer independently retain the common
length-twenty-three root and twenty-two operations plus one return-edge fuel
units. Missing or reordered establishments, root/index/length drift,
cleanup-order drift, and other prefix drift remains fail closed without runtime
liveness state or a cleanup loop.

The next bounded successor admits `[T; 24]` under the same restrictions and
establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`, `9`,
`10`, `11`, `12`, `13`, `14`, `15`, `16`, `17`, `18`, `19`, `20`, `21`, then
`22`. Ordinary abandonment cleans the twenty-three zero-ABI element
occurrences in order
`[22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`;
Terminal and every native artifact layer independently retain the common
length-twenty-four root and twenty-three operations plus one return-edge fuel
units. Missing or reordered establishments, root/index/length drift,
cleanup-order drift, and other prefix drift remains fail
closed without runtime liveness state or a cleanup loop.

The following bounded successor admits `[T; 25]` under the same restrictions
and establishes literal indices `0`, `1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`,
`9`, `10`, `11`, `12`, `13`, `14`, `15`, `16`, `17`, `18`, `19`, `20`, `21`,
`22`, then `23`. Ordinary abandonment cleans the twenty-four zero-ABI element
occurrences in order
`[23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]`;
Terminal and every native artifact layer independently retain the common
length-twenty-five root and twenty-four operations plus one return-edge fuel
units. Missing or reordered establishments, root/index/length drift,
cleanup-order drift, and length-twenty-six or wider prefixes remain fail
closed without runtime liveness state or a cleanup loop.

Named record and case literals generalize the construction half without
changing completed-value ownership. Field expressions establish exactly once in
authored literal order. Ordinary abandonment cleans that established prefix in
reverse establishment order; after successful completion, the aggregate owns
its fields structurally and later cleanup returns to recursive reverse
declaration order. Partial call-argument staging uses the same reverse-
establishment rule. Canonical static field order and physical layout do not
participate in this schedule.

An internal call may now continue that same whole-root result through one
explicit terminal operation-result place. The checked slice is intentionally
narrow: one final direct call, one whole linear qualified argument, one exact
checked callee with one whole result claim, and an immediate caller return.
`CallStructural` records both the ordinary caller-to-callee claim transfer and
the exact callee-to-caller returned-claim map. The operation result declares its
structural signature and caller claim binding independently, so canonical
decoding and verification can reject producer, type, qualification, path, or
claim drift. Custody becomes live in the caller only after successful return;
crash creates no result and fuel suspension cannot replay a transfer. Projected
or multi-claim calls, bodyless structural results, local staging, and wider
native aggregate ABI lowering remain later slices rather than implicit
fallbacks.

Checked facts retain the first per-edge cleanup subset for ordinary named
transitions: each exact source-state/statement/target row names the whole,
claim-free affine parameter positions discarded on that arm after subtracting
checked transfers. A separate checked-only row now covers one attached,
two-state, Unit-returning unconditional jump with exactly one non-self affine
root: an unqualified two-field structural record may move one whole direct
field to the sole exact-same-type successor parameter and retain the sibling as
the one maximal no-code residual. It is disjoint from whole-root rows, and all
existing Terminal consumers reject that edge coordinate because Terminal has
no path-segment successor vocabulary. Wider/nested records, multiple roots,
claims, contracts, nominal cleanup, and executable replay remain fenced.
Other locals, projection shapes, nominal cleanup, and any otherwise incomplete
shape publish no row. A first terminal
consumer composes those rows with exact source-handle-free state signatures and
whole-parameter transfer maps for attached, multi-state Unit machines. It
accepts only unconditional, acyclic, single-predecessor custody lineages and
requires transfers plus reverse-order cleanup to partition every source
frontier; stale types, positions, cleanup, joins, cycles, and reordered custody
reject. One narrow conditional producer now selects two ordered successors from
a retained Boolean scalar input and independently reconstructs each arm's
whole-parameter transfer/cleanup partition. A decision state may follow
an unconditional prefix, and one arm may contain a second decision; a third
conditional state remains fenced.
Unconditional jumps and conditional
arms may forward direct scalar inputs into typed successor parameters; terminal
edge semantics materialize those arguments before cleanup. One bounded diamond
may reconverge when both predecessors reconstruct the same ordered custody
frontier; scalar values bind through the join's typed parameters. Divergent
frontiers, wider joins, and cycles reject. Computed guards or successor values
and wider conditional graphs remain fenced. The complete
`EdgeCleanupPlan`, contextual
cleanup contracts, repeated-cycle composition, and the retained whole-edge
conservation witness remain CML4 work.

Scalar-result materialization does not change that ordering. A first attached
one-state source slice now evaluates an ordered prefix of immutable primitive
locals and a return through checked branch-free scalar expressions. Integer
work uses landed literals and the terminal integer vocabulary; Boolean work uses
constants, negation, equality, and integer comparisons. Expressions may name
primitive state parameters and already materialized locals. A retained source-
position partition maps primitive inputs into the dense scalar namespace and
keeps affine custody in the disjoint structural namespace; the two maps must
cover every authored parameter exactly once. The producer rechecks that
partition, every local coordinate and carrier, the return, the structural
signature, and the cleanup row before assigning terminal identities, then
reconstructs exact-operation proofs before publication and performs
reverse-declaration cleanup on the return edge. Final short-circuit Boolean
returns preserve the full frontier across internal decisions and repeat that
exact cleanup on every terminal value leaf, which the verifier checks
independently. Calls, mutable or non-scalar locals, claims, and richer exits
remain fail-closed rather than weakening the frontier model. Branch-free
primitive work may surround and separate any finite sequence of short-circuit
Boolean local stages, and each stage may contain a finite `&&`/`||` decision
tree of arbitrary nesting: each prefix preserves custody through every decision
edge, each Boolean value enters one typed convergence parameter, and cleanup
remains only on the subsequent return. If that return is itself short-circuit
Boolean control, each terminal value leaf repeats the same exact cleanup.

Consuming calls are classified from result flow: if a by-value `self` call
returns a type carrying the obligation, it transfers rather than terminally
consumes. One unambiguous moved input preserves its origin into the result;
claims transferred through a target state's direct path or record-constructor
result preserve their callee identities and origins when the caller binds the
matching paths. Opaque multi-resource calls without a normalized output map
reject. Content-bearing n-to-m transformations additionally discharge the
selected compiler-owned algebra's conservation theorem.

Generic conditional sums resolve payload multiplicity through symbol-keyed
type-argument substitution. Consequently `TaskOutcome<LinearT>::Returned` and
`StartOutcome<T, LinearArguments>::Rejected` carry live debt, while their
payload-free or affine cases do not; generic template parameters cannot launder
a concrete linear substitution.

A backend may erase multiplicity only after checked ownership and cleanup
evidence is complete. Proof/debug artifacts must retain the conservation
witness; see `architecture/semantic_taxonomy_representation.md`.

## Acceptance register

1. Establishing one linear value and moving it through several bindings still
   produces exactly one live obligation.
2. Implicit zero-fill creates no obligation; explicit construction of a valid
   all-zero linear value creates one.
3. A live linear value at ordinary scope exit is rejected unless its exact
   owner-authorized automatic plan is a valid terminal disposition there.
4. Every branch either transfers or consumes the same linear obligation; mixed
   treatment is rejected.
5. `Empty | Live(Task<T>)` tracks the obligation only in the live case.
6. A record, sum payload, or generic container cannot erase a contained linear
   obligation.
7. `Task<T>` must be explicitly finished or transferred; requesting
   cancellation alone does not discharge it, and automatic cleanup never
   blocks.
8. Shared and exclusive loans keep their own permission rules rather than
   being forced into exact linear reconciliation.
9. Linearity works for a fixed-size acknowledgement token without any
   dependent-type feature enabled.
10. A linear claim participates only in whole-claim conservation unless its
    exact qualification publishes a compiler-owned content projection.
11. A split whose children overlap or duplicate content rejects even when each
    child is individually contained and their scalar measures add up.
12. Permission attenuation cannot be undone by merge; authority that must
    return is represented by a claim or loan.

## Deferred design spaces

- Additional content algebras beyond intervals and counted quantities.
  Correspondence-bearing virtual/physical decomposition requires a compact
  canonical symbolic mapping algebra with decidable containment, restriction,
  equality, and separated composition; independently conserving the two
  projections is unsound because it permits their association to be swapped.
- Fan-out obligations requiring one value or claim to reach exactly `n`
  distinct destinations. `CountedQuantity(n)` instead models a spendable pool
  of `n` units and does not settle that separate semantics.
- Quantitative operational resource accounting remains a separate algebra from
  content-bearing claim conservation.
- Dependent-linear buffer ergonomics after the core checker exists.
- The first conservative cross-suspension loan subset. Four-axis carry policy
  is settled independently; this item is the remaining borrow-rule and
  implementation work for values live at suspension points.

The first content-algebra customer is Extent split/merge: splitting
consumes one parent range authority and returns disjoint child authorities;
compatible-common-lineage merging consumes exactly composing children and
restores their parent content.
Arena-backed task-pool leases reuse the conservation discipline without
conflating allocation permission with range authority. General owned `LinBuf` splitting and
quantitative effect members come later.

The compiler foundation now has an executable `psi-extents` conservation
model: roots come only from one-shot admitted grants; split is exact; merge is
restricted to compatible children of the same split; attenuation only removes
normalized rights; and consuming failures return their authority inputs. This
does not replace the Omega `[linear]` checker work. Source integration replaces
the temporary sibling-only restriction with compatible common root lineage,
algebra-denominated receipts, one normalized claim-content projection, and the
generic n-ary conservation witness. Permission attenuation stays orthogonal:
merge never restores a discarded permission, and authority that must return is
a separate claim or loan.
