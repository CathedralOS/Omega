# Dynamic dispatch

Dynamic calls retain the descriptor or parameter slot that selects their
realization. They do not replace that selection with a static callee or a copy
of its source expression.

## Local selection and call shape

`dyn Trait` stays inside one artifact and uses its internal convention, not a
boundary calling plan. A named whole-trait conformance supplies one closed map
with exactly one row for each inherited `(declaring trait, complete requirement
overload)` slot. Each row selects the conformance's member, explicit machine
reference, or own default instantiation. The complete overload includes its
normalized parameter signature and dispatch-bearing result-domain set.

Calls retain the exact requirement symbol, including inherited requirements.
Same-spelled inherited requirements are not one slot; ambiguous unqualified
calls reject. An independent per-requirement satisfier does not form a whole
dynamic conformance. Neither carrier-qualified machine names nor uniquely visible
machines supply missing rows.

Requirements and realizations are static machines with explicit parameters,
under the [ordinary machine contract](../language/machines.md#static-declarations-and-explicit-state).
Selector-only `dyn Trait<Arguments...>` selects a named implementation at runtime.
It does not capture state or hide a trait argument. The explicit
[borrowed capture form](#borrowed-captures) binds named arguments and erases one
trait type argument without introducing a receiver.
For example, `dyn CounterOperations<CounterData>` can select normal or
saturating operations over the same explicitly passed `CounterData`.
`dyn Convert<Image, Text>` can select different Image-to-Text implementations;
neither `Image` nor `Text` is a distinguished subject.

A selection is constructed from an exact named implementation:
`let operations: dyn Ranked<Card> = PowerOrder as dyn Ranked<Card>;`.
Calling `Ranked::rank_value(operations, &card)` explicitly supplies the
implementation selector and then the requirement's declared value arguments.
The selector is a dispatch operand, not an inserted requirement parameter or
a data argument forwarded to the selected machine. The checked call retains
it separately from the ordinary argument list. A direct static selection uses
the named implementation or a static evidence binder instead.

Coercion retains the target trait application, exact implementation and
normalized rows. It does not coerce a data place into a trait object or search
for implementations of that place's type. A bare dynamic parameter accepts
fitting eligible maps without selecting one by visibility. A bodyless
declaration without a normalized map is not a dynamic candidate.

The requirement after ordinary argument substitution owns the call shape.
Every realization matches that shape; a representative candidate cannot choose
it for the rest. Descriptor forwarding, rebinding, joins and storage preserve
the actual selected implementation. Each predecessor at a join supplies its
own selector; never invent a representative implementation or mixed table.
Data arguments keep their own independent place, lifetime and ownership facts.

Named decision `dynamic-caller-subject-parameter` is settled by this model:
a dynamic caller has ordinary explicit parameters and an explicit selection
operand, with no attachment or synthetic receiver. No parameter position is
privileged. Terminal Psi must retain selection and value arguments separately.

## Source eligibility and operational envelopes

Selector-only `dyn` values carry implementation selection, not owned or borrowed
user state. Borrowing a selector does not borrow an implied data instance. Copying
or storing it grants no authority over any separately passed data. A proof-only
selection with no executable rows erases under the ordinary evidence rules.
Selector-only construction creates no data-instance allocation or cleanup.
Captured values instead retain their explicit borrows and their loan obligations.

After binding trait arguments and any exact family tuple, the requirement's
parameter/result representations must be concrete, returned borrow lifetimes
must be expressible from inputs, and the public contract must name no
implementation-private identity. Zero, one, or multiple data parameters and
owned results are allowed under their ordinary contracts. There is no
one-subject restriction or `Self` eligibility rule. Requirement-local generic
parameters remain ineligible except for the closed value families below.
Boundary-machine requirements use their boundary calling plan, not local
dynamic tables. Eligibility remains per requirement.

Static machine requirements may bind implementation evidence under
[generic machine requirements](../language/generics.md#generic-machine-requirements).
That does not make an unrestricted requirement-local type, machine or evidence
binder dynamically eligible. Check the attempted dynamic interface use at source
checking, before native table construction: it must supply eligible concrete
requirement applications and complete declared row coverage. Reject an open
generic method surface rather than guessing entries from current callers or
deferring the missing call shape to a linker or runtime fault.

Generic trait arguments are different: binding them before forming, for example,
`dyn Worker<u32>` can leave a concrete nongeneric method signature. Runtime
implementation selection still uses that closed interface's table even though
its generic arguments are static. A Build-selected generic provider instead
retains static application and provider selection, not local `dyn` dispatch.
The finite value families below remain the explicit exception for eligible
requirement-local value binders; this decision does not extend their roster to
arbitrary type or evidence applications.

Only the explicit borrowed capture form below hides a trait type argument.
It supports heterogeneous borrowed state without an existential keyword,
implicit boxing, owned erased state, or an unrestricted runtime type facility.

The normalized operational contract must fit its per-requirement dynamic
envelope: service reach, direct synchronous invocation, mutation summary,
capability demands, suspension, blocking, failure, termination, quantitative
resource ceilings, and guarded crash routes. Carry is a property of the dynamic
value, not one requirement. One selected conformance supplies the whole surface;
tables cannot combine rows from unrelated conformances.

The envelope is compile-time information and adds no runtime words. Under
[`known-dynamic-contract-narrowing`](../language/effects.md#known-dynamic-selections),
a check-time coercion retains the selected conformance's available checked
envelope, not a private-body estimate narrower than its published contract.
An unknown selection retains the requirement ceiling. At joins, possible demands
combine by union or maximum and guarantees by conjunction or intersection.
Carry permissions intersect; termination survives only if every alternative
guarantees it. `suspend` and `block` acknowledgements use the retained envelope,
not the wider base declaration when narrower evidence is retained. Boundary
provider selection at Build or run time supplies no such coercion evidence.

An unannotated dynamic parameter is implicitly polymorphic over fitting
envelopes. Only requirements reachable through its call graph contribute to the
inferred contract, including forwarding into transitive calls. Its generic body
must be valid for every fitting input envelope; one caller's known selection
does not become an unconditional assumption. Only retained checked dependencies
or specialization can narrow that application. Storing the value
instead requires the storage type's bound. This changes static contract checking,
not runtime representation, and does not require code monomorphization.

## Borrowed captures

A captured dynamic value pairs one exact named implementation with explicitly
bound borrowed arguments. This lets one container hold operations over different
state types while keeping each implementation paired with the state it accepts.
The following source form is ratified; compiler delivery and acceptance are
tracked in the [implementation plan](../../drafts/designs/captured_dyn.md).

```omega
trait Metric<S> { machine value(state: &S) -> u64; }

// QueueDepth satisfies Metric<Queue>; CacheHits satisfies Metric<Cache>.
let metrics: [dyn Metric<_>; 2] = [
    QueueDepth as dyn Metric<_> { state: &queue },
    CacheHits as dyn Metric<_> { state: &cache },
];
let value: u64 = Metric::value(metrics[0]);
```

Here `_` erases one trait type argument after the concrete conformance and its
arguments have been checked. It is not inference of one common state type for
the container. Other trait arguments remain explicit or ordinarily inferred.
V1 captures borrowed state only, within one Fused artifact. Selector-only
construction and calls remain unchanged.

### Binding and erasure eligibility

The binding map identifies requirement parameters by name, never by position
or an implicit receiver. A short name must resolve to one exact parameter slot;
qualification by the declaring requirement is needed where names collide.
For example, `Metric::value::state` names `value`'s `state` parameter.
Overloads retain their complete requirement identity; unresolved ambiguity
rejects rather than binding several same-spelled parameters accidentally.
Capture identity records that resolved slot, not just its source spelling.

Every parameter whose type depends on the erased argument must be bound, across
all inherited requirements and admitted finite family rows. Each such parameter
must be a borrowed reference in V1. Missing, duplicate, extra, mismatched, or
owned captures reject. `compare(left: &S, right: &S)` therefore needs both
bindings; the caller cannot later supply a value of an unknown `S`. Every capture
expression is checked and evaluated once at construction against the concrete
substituted parameter type. Ordinary alias and loan rules apply across bindings.

At a call, the dynamic value supplies the selected requirement's bound arguments;
the caller supplies the remaining parameters in declaration order. The selected
machine still receives its full explicit argument list. A call cannot replace a
capture with a differently typed argument or exchange state between entries.
Capture construction does not discharge future calls' preconditions forever:
each invocation checks the complete contract against current captured places,
storage versions and supplied arguments. Hidden concrete types grant no extra
facts; available contracts must justify the call for every remaining selection.

After removing the bound parameters, the callable interface must have a concrete
common shape. No result may expose the erased type, including `&S`; an expressible
lifetime alone does not make a hidden result type usable. Concrete results and
their input-relative loans remain eligible under ordinary rules. No implicit
result repackaging, unrestricted generic virtual methods, or new proof admission
rule follows from erasure. Operational envelopes retain their existing rules.

### Loans, storage and runtime insertion

A captured value retains its selector and the exact borrowed places. Lifetime,
multiplicity, access qualifications and custody survive fields, parameters,
returns, joins, moves, containers and helper calls. Shared captures are copyable
only when their complete contents permit copying. Mutable captures are not
freely copied; invocation uses ordinary exclusive access and reborrowing without
consuming a reusable capture merely to call it. Result loans can extend that
access. Binding or erasing a type cannot widen authority or extend a lifetime.

Runtime branches and helpers may append captured values to a mutable container.
The number, order and selected variants of its entries are runtime data; the
roster of possible variants is static. All producers and consumers of that
captured type use the same artifact roster, not a roster inferred separately for
each initializer or container. A helper cannot store a borrow of its own local
past that local's lifetime. Removing an entry transfers its loans with the value;
consuming it can end those loans under the existing rules, without requiring
destruction of the container. Removal alone does not release a loan still held
by the removed value, a copy, or an outstanding result reborrow.

### Roster identity and lowering

The compiler collects a finite roster from admitted capture constructions in
the closed artifact, including concrete generic instantiations. It does not
enumerate every implementation merely because it is visible. Each entry retains
the exact conformance application, concrete erased type, normalized requirement
rows and capture bindings. Repeated construction with different runtime state
of the same entry does not add a variant. Failure to close a finite roster
rejects; runtime insertion cannot introduce an unadmitted implementation/type.

The captured interface identity includes the trait application with its erased
position and resolved binding schema. Ordinary lifetime/access/envelope
constraints still govern compatibility. Canonical roster order and commitment
are independent of source traversal. Finalize the roster before representation
and dispatch, without imposing a particular compiler stage number.

V1 lowers the value as a compiler-derived closed sum of borrowed environments
and a matching dispatch over its roster. The selected case retains both state
and implementation through checking, Terminal Psi, interpretation and native
execution. Each case expands into the concrete row's full argument shape.
Verification binds the roster, tag, selected row, captures and result contract;
an unknown tag or mismatched capture/row rejects. This is not the scalar-only
selector representation described below. Target realization chooses physical
tags and dispatch instructions; no two-word layout or jump table is promised.

All values of the same captured interface in a Fused artifact share its finalized
roster. A new admitted entry changes roster identity even if physical size stays
the same; every dependent representation and dispatch site must be regenerated
consistently. A new site using an existing entry does not enlarge the roster.
This artifact-wide dependency does not automatically include Independent
components elsewhere in the application.

## Complete application identity

### Finite generic method families

An explicit finite value family under the
[generic specialization contract](../language/generics.md#finite-specialization-boundary)
may expose requirement-local value binders through a dynamic implementation
interface.
Unrestricted local type/machine binders or ranges requiring arbitrary enumeration
do not acquire dynamic eligibility. Every tuple must close the method's remaining
local parameters and yield an eligible concrete call shape and contract.

The declaring requirement, not the selected provider or set of current callers,
owns the complete family roster. One named conformance supplies all its rows:

```text
(declaring trait, complete requirement overload, canonical value tuple)
```

A nongeneric requirement uses the empty tuple. A generic body or explicit
existing implementation can provide several rows through checked specialization;
authors need not spell width-suffixed method names. Partial provider coverage
rejects. If unsupported execution is intended, the public result contract must
say so and every row must implement that outcome rather than disappear.

Canonical tuple order determines row order independently of source disjunction
order, runtime values, and provider traversal. Each row retains its own exact
parameter/result shape, operational envelope, implementation, and adapter.
Inherited requirement identity and the selected conformance remain intact.
Tables cannot combine widths from unrelated conformances. Expanding a public
family changes its contract and coverage obligations, requiring ordinary
compatibility checks rather than silently adding unbound slots.

A static family argument selects the exact row; a runtime-capable family call
selects a row after proving its argument belongs to the roster. `const` arguments
still require a literal/static selection through the enclosing checked dispatch.
The selected tuple governs operands, result representation, and continuation;
forwarding or storing a prepared value must not disconnect its index from its
actual data and callable selection.

Use a common concrete result, keep the result inside the selected static branch,
or author a finite sum/eligible descriptor with an explicit ownership contract.
A bare unknown-layout `Prepared<runtime_width>` does not acquire a uniform ABI
from table membership. This introduces no implicit allocation, existential
packaging construct, or general runtime type representation.

Refinement, table completeness, call-site selection, and artifact replay include
the exact tuple. Width/shape/slot substitutions reject even when byte widths
coincide. Known finite indices do not make external plugin bodies statically
known or remove their ordinary admission and resource obligations. Current
nongeneric dispatch support is not evidence of implemented family expansion;
the implementation migration is tracked beside
[source staging](../../../omega-rust/psi/compilation/pipeline/README.md#value-generic-staging).

### Selection retention

Each selection retains the complete canonical `ClosedConformanceApplication`:
telescope, ordered requirement-row map, callable registry, report coordinate,
and strong commitment. Selecting one row does not permit dropping or reordering
unselected rows. Verification rejoins the application to the exact selected row,
callable, trait arguments, explicit argument types and access, result, and service
reach.

A rebound descriptor retains initializer and latest selections and their
version relationship. Rebinding to a different named conformance preserves the
exact dynamic trait application, telescope and ordered requirement roster,
within the storage's declared operational envelope. It does not rebind any
separately supplied data argument. The two applications have independent
commitments. Only
the latest supplies the executable table; an overwritten initializer remains
identified without inventing an unused table.

Call composition, reachability, verification, interpretation, optimization
validation, and logical-work derivation resolve the same selection. Orphaned,
duplicate, reordered, retargeted, or static-call-substituted records reject.
Knowing the latest selection is not permission to devirtualize its call.

## Call and forwarding roles

The following scalar-selector rules govern selector-only values; captured values
expand their environments under the roster rules above before applying the
concrete requirement's call contract.

The selector-only `Impl as dyn Trait<Args>` produces `SelectImplementation`,
naming that implementation's table. Each selector-only call is one `CallDynamic`
naming its interface slot: the selector is the dispatch operand, and the
slot's call shape governs the ordinary arguments and the result. Where the
selector came from (a local selection, a rebinding, a join, a stored field or
a forwarded parameter) is ordinary value flow of one scalar word, not a
separate call representation. An outer call that forwards a selector and the
helper's dynamic call remain two independently identified operations.

The slot's call shape is the requirement instantiated under the selector's
trait application. Checking plans each dynamic call against that shape with
the selector as its first operand, so argument custody, borrows and write
frames treat the call like any call to a bodyless checked callee. Every row
of every table realizes the shape exactly.

Each slot also retains the requirement's complete owned disposal-claim
frontier as canonical `(structural argument index, typed relative path)` rows.
The index is structural-lane-local; neither the selector nor preceding scalar
formals occupy it. An affine record can contain several linear descendants,
and case paths retain guards, so root multiplicity cannot replace this roster.
Every selectable implementation realizes these paths under its own private
entry claim identities. Those identities are not part of the public slot.

Statement and computed selector calls carry the caller's actual live claims
through the same exact paths. Copies, projections and operand staging retain
their established owners; selection grants no data custody. Independent
ownership checking matches the live frontier and consumes each transferred
identity once. An omitted guarded row requires independent discriminator
evidence of inactivity, not an absent claim. Runtime dispatch checks the
selected callee's active paths against captured arguments before transfer.

Unit calls have `OperationResult::Unit`: no invented value ID or result home.
Scalar results bind an exactly typed value through the ordinary caller;
Boolean results do not become integer-shaped placeholders.

Structured results retain the requirement's exact type, multiplicity and
qualifications without naming an implementation's private result home. Each
invocation publishes its own result storage and claim identities through the
ordinary return operation. The slot catalogues possible result claims by typed
relative path. Every successful return supplies each unguarded claim; guarded
claims follow the returned case and its independently established custody.
An implementation need not return every alternative of a public sum. A
requirement's possible case frontier is therefore not a demand that every row
exercise every possible result claim. Returned reference origins require the
same public input-relative contract as the source result; a row's private
formal places cannot stand in for that contract.

## Native tables and replay

For selector-only values, native realization represents the selector by its
implementation table reference or an equivalent target representation preserving
that identity.
There is no canonical `{ instance, table }` pair: remove the retired instance
field and its receiver adapters. Descriptor and scalar-result storage do not
overlap. Explicit borrowed arguments obey [reference identity](structural_access.md)
independently; copying a selector cannot copy or extend a borrowed referent.

Explicit captures follow the borrowed-environment sum above, not the retired
implicit instance field. No hidden receiver is added to a requirement.

Immutable tables may deduplicate identical complete applications by strong
identity. Emit one slot per canonical row. A row with an executable callable
binds its exact function symbol; a semantic row without a callable retains a
zero trap slot. Do not compact it away or invent a function.

Independent object and final-image replay checks descriptor bytes, table-address
relocations, selected-slot load/call, function-slot relocations, and final data.
Retained evidence binds complete table/slot projection and each call's
application, sources, selected slot, realization, and code interval. Installed
replay preserves the final text/data layout and section gap, exact relocation
addresses, and pre-relocation and final bytes. This immutable-table contract
does not imply mutable data or BSS support.

## Mutation-bearing realizations

A realization's ordered stores are distinct from its return expression. Each
store retains the exact explicit argument/path, primitive field, typed value,
and source order; subsequent reads retain their own identities. Lowering and replay
reconstruct all field offsets from declarations. A subloan of an unrestricted
mutable root remains unrestricted, not an invented linear entry claim.

The [structural store contract](structural_access.md#store-vocabulary) governs
access and write visibility. Store emission followed by a successful dynamic
call is insufficient unless the caller's original referent reflects the writes.

Current source and native subsets are described beside
[Terminal-to-abstract lowering](../../../omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/README.md#dynamic-dispatch).
They are implementation limits, not additional source semantics.

## Resource and component boundary

A dynamic call accounts for selector dispatch, table adapter, concrete physical
argument shape, and selected satisfier demand. A suspension-capable caller shape
still costs its frame and structural work when a particular satisfier never suspends.
The selected implementation may meet a suspension promise while exceeding a
separate stack or work ceiling.

Captured values additionally account for their environment storage, case
selection and bound-argument adaptation. Erasure does not remove those costs
or the underlying loan, suspension and resource obligations.

An occurrence-specific, whole-artifact devirtualization may replace a nonescaping
selection/call with its exact realization and original explicit arguments.
It must retain the realized direct-call cost, not erase it because the target is known. General
escaping descriptors still need their actual table and adapter. Private
realization footprints compose into the enclosing root's evidence without
inventing separate boundary contracts.

Local tables do not cross replaceable component boundaries. Those calls use
the requirement's evaluated boundary plan and entry contract; a local proxy may
adapt the component binding back into a dynamic value inside its consumer.

Captured values follow this same boundary: V1 keeps both their roster and
borrowed environments local to the Fused artifact. Their conformance/code,
roster/representation, and captured-place custody dependencies must be published
into the [replacement closure](../build/component_publication.md#replacement-granularity).
Changing a referent's size need not change a borrowed environment's size, but
does not erase the dependent code or custody edges. Quiescing calls alone does
not dispose of captures; old values must be drained, retained with their era,
or consumed and reconstructed after an explicit state migration.
