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
`dyn Trait<Arguments...>` selects a named implementation at runtime. It does
not package an instance of an implementing data type or hide a trait argument.
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

Runtime `dyn` values carry implementation selection, not owned or borrowed user
state. Borrowing a selector does not borrow an implied data instance. Copying
or storing it grants no authority over any separately passed data. A proof-only
selection with no executable rows erases under the ordinary evidence rules.
There is no data-instance allocation or cleanup obligation created by `dyn`.

After binding trait arguments and any exact family tuple, the requirement's
parameter/result representations must be concrete, returned borrow lifetimes
must be expressible from inputs, and the public contract must name no
implementation-private identity. Zero, one, or multiple data parameters and
owned results are allowed under their ordinary contracts. There is no
one-subject restriction or `Self` eligibility rule. Requirement-local generic
parameters remain ineligible except for the closed value families below.
Boundary-machine requirements use their boundary calling plan, not local
dynamic tables. Eligibility remains per requirement.

Hiding data types is a separate packaging facility, not part of implementation
selection. This contract introduces no existential type binder, heterogeneous
data container, or implicit boxing. A future such facility must preserve the
relationships between hidden types and the values accepted or returned.

The normalized operational contract must fit its per-requirement dynamic
envelope: service reach, direct synchronous invocation, mutation summary,
capability demands, suspension, blocking, failure, termination, quantitative
resource ceilings, and guarded crash routes. Carry is a property of the dynamic
value, not one requirement. One selected conformance supplies the whole surface;
tables cannot combine rows from unrelated conformances.

The envelope is compile-time information and adds no runtime words. Coercion
retains the selected conformance's exact envelope. At joins, possible demands
combine by union or maximum and guarantees by conjunction or intersection.
Carry permissions intersect; termination survives only if every alternative
guarantees it. `suspend` and `block` acknowledgements use the retained envelope,
not the wider base declaration.

An unannotated dynamic parameter is implicitly polymorphic over fitting
envelopes. Only requirements reachable through its call graph contribute to the
inferred contract, including forwarding into transitive calls. Storing the value
instead requires the storage type's bound. This changes static contract checking,
not runtime representation, and does not require code monomorphization.

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

`Impl as dyn Trait<Args>` produces a selector with `SelectImplementation`,
naming that implementation's table. Every dynamic call is one `CallDynamic`
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

Unit calls have `OperationResult::Unit`: no invented value ID or result home.
Scalar results bind an exactly typed value through the ordinary caller;
Boolean results do not become integer-shaped placeholders.

## Native tables and replay

Native realization represents the selector by its implementation table
reference or an equivalent target representation preserving that identity.
There is no canonical `{ instance, table }` pair: remove the retired instance
field and its receiver adapters. Descriptor and scalar-result storage do not
overlap. Explicit borrowed arguments obey [reference identity](structural_access.md)
independently; copying a selector cannot copy or extend a borrowed referent.

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

An occurrence-specific, whole-artifact devirtualization may replace a nonescaping
selection/call with its exact realization and original explicit arguments.
It must retain the realized direct-call cost, not erase it because the target is known. General
escaping descriptors still need their actual table and adapter. Private
realization footprints compose into the enclosing root's evidence without
inventing separate boundary contracts.

Local tables do not cross replaceable component boundaries. Those calls use
the requirement's evaluated boundary plan and entry contract; a local proxy may
adapt the component binding back into a dynamic value inside its consumer.
