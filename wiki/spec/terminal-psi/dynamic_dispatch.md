# Runtime callable dispatch and package evidence

Runtime calls select machine values stored in ordinary data. Their callable
contracts govern explicit arguments and results. `some` packages preserve the
relationship between an unknown state type and operations accepting that type.

Ratified 2026-11-06. The source `dyn` selector, captured `dyn Trait<_>` form and
mandatory artifact-wide derived-sum roster are retired. This path remains the
dispatch contract so existing links keep their owner. Source rules live in
[machine values](../language/machine_values.md) and [some packages](../language/some_packages.md).

## Local selection and call shape

A callable descriptor identifies a closed machine application under an exact
callable contract. It does not require a whole-trait table, receiver or implicit
state. Taking an explicitly named conformance member retains its exact
requirement/implementation application, but other fields of an operations record
need not come from that same conformance unless a stated law demands it.

Keep callable selection separate from ordinary arguments. The callable type owns
the full call shape: argument order, representation, qualifications, access,
lifetimes, effects and result. No representative target may choose that shape
for other values of the type. Forwarding and joins retain the actual descriptor;
they do not replace it with a static callee or reconstruct it from source text.

## Source eligibility and operational envelopes

An executable callable has a closed callable signature. Unrestricted local type,
machine or evidence binders cannot be called through an unknown runtime value.
Bind static arguments before constructing the callable; the explicitly finite
value-family rule below remains separate. `some` witnesses in callable arguments
are abstract but related to the same package opening, not unrestricted runtime
generic dispatch. Their physical arguments must have the admitted uniform shape.

The normalized callable contract includes service reach, synchronous invocation,
mutation summary, capability demand, suspension, blocking, failure, termination,
quantitative resources and guarded crash routes. Calls check those obligations
against current argument versions, not facts frozen at construction.
Its service and crash ceilings are finite and authored; a runtime callable
type states both, with `reaches none` and `crashes none` for empty.
Resolve static callback and conformance-member reach dependencies before
constructing the closed callable. An unresolved open row cannot become a
runtime callable ceiling.

Known callable selection can retain its checked operational envelope under
[known-selection narrowing](../language/effects.md#known-dynamic-selections).
Joins union possible demands and intersect guarantees. Unknown selections use
the declared ceiling; no caller set, code-address guess or devirtualization
prediction strengthens a public contract. Callable-parameter dependencies retain
the existing checked inference discipline. Storage obeys its declared bound.

## Borrowed captures

The former captured-`dyn` facility is superseded by ordinary data containing
borrowed fields, machine values and `some` binders. There is no capture-map
syntax or hidden receiver. This heading retains links from the retired design;
the normative opening and no-escape rules are in [some packages](../language/some_packages.md#opening-by-stable-subject).

## Complete application identity

Machine-value construction retains the exact selected machine application,
static arguments, originating conformance/requirement where applicable,
callable contract and refinement evidence. A call names the current descriptor
value, callable contract and actual arguments. No complete trait table is
invented for a structural callable. Independent record fields retain independent
selections; cross-field laws require separately retained checked evidence.

### Finite generic method families

The existing explicitly finite value-family facility remains available through
callable contracts with a declared canonical family roster. It is not a roster
of machine targets or package witness types. Each permitted tuple closes the
remaining value parameters and has a concrete argument/result shape and contract.
Unrestricted local type/machine binders remain ineligible.

Construction must supply the entire declared family through an exact closed
machine family; current callers cannot shrink it. The declaration owns canonical
tuple order and coverage. Each row retains its tuple, callable, shape and
refinement evidence. Runtime selection proves the tuple belongs to that family;
`const` arguments retain static selection. Mixed-row coverage, missing tuples,
or a shape/result from another tuple reject.

Different tuple results need a common concrete result or an explicit authored
sum/package with valid custody. Membership in a finite family does not make an
unknown-layout result uniform. Source changes to a declared family affect its
contract and dependents; adding another machine satisfying an unchanged
callable contract does not create such a change.

### Selection retention

Reassignment records the new descriptor and its storage version. Call checking
uses that version; an old initializer cannot supply current selection evidence.
Moves, fields, arrays, joins and containers preserve selection and custody.
Orphaned, duplicated, retargeted or static-call-substituted evidence rejects.

For `some` data, retain a package construction witness and each stable opening's
subject/version identity. All dependent projections refer to that opening.
Reassignment retires old associations; canonical indexed subjects include their
captured index value. Relocation and overlapping writes respect outstanding
loans. Equal field sizes cannot substitute for a shared witness.

## Call and forwarding roles

Checked output supplies explicit callable construction/call plans, package
construction/opening/projection identities, argument/reborrow maps, postconditions
and custody transfers. Lowering consumes them without re-searching source ASTs
or teaching the backend a special `record.operation(record.state)` pattern.

Every indirect call retains its callable contract and actual descriptor. Expand
no receiver and drop no argument because the callable came from a field. Argument
and result evidence uses ordinary canonical places and exact relative paths.
Inherited/overloaded requirement identity is retained where the selected
machine originates in a named conformance.

Owned disposal frontiers retain every guarded structural argument path, with
independent evidence for inactive cases. Each transferred identity is consumed
once. Mutable references reborrow under ordinary rules; a reusable call does not
consume its stored reference. Unit results invent no value home. Scalar results
keep their exact type; structural results retain qualification, multiplicity,
claim frontiers and public input-relative borrow origins. Each call publishes
its own result storage and custody, never a private target's formal place.

## Native tables and replay

The baseline uses indirect descriptors under the callable's common contract.
Operations tables are ordinary authored records, possibly stored separately
and borrowed by a package. There is no required generated trait vtable, target
roster, witness tag or fixed word count. Borrowed `some` environments have a
uniform physical layout; logical witnesses do not become source-visible RTTI.

Replay binds descriptor construction, exact callable target, contract identity,
argument/result shape, package pairing evidence, relocations and realized code.
Function addresses alone are not proof of type or authority. Independent
verification rejects forged descriptors, stale openings, mismatched witnesses,
wrong contracts and missing custody transfers. The interpreter and native
realization must agree, including mutable referent writes and returned loans.

The [encoding contract](encoding.md) records current wire forms separately from
this ratified target. Implement producer, codec, interpreter, verifier and native
consumers together; do not silently reinterpret legacy selector-table bytes as
machine values or invent unsupported operation tags in documentation.

## Mutation-bearing realizations

An indirect target's ordered stores retain exact explicit argument paths, values
and source order. Subsequent reads observe those writes through the caller's
actual referents. Reconstruct field offsets from the relevant concrete
declarations at realization; `some` does not authorize field access through an
unknown abstract type. The [structural store contract](structural_access.md#store-vocabulary)
and ordinary reference identity continue to govern aliases and visibility.

## Resource and component boundary

Account for descriptor/environment storage, indirect dispatch, argument
adaptation, suspension frames and selected callee demand. Devirtualization may
replace a proven selection with a direct call or finite dispatch, retaining
its costs and dependencies. A compiler's closed target-set analysis is not a
mandatory global source roster or a guarantee that an indirect call disappears.

Local machine descriptors and packages containing them do not cross an
independently replaceable component boundary unchanged. Use the admitted boundary
plan and a local proxy when a local callable view is needed. Callable code and
borrowed state publish their code, layout and custody dependencies into
[replacement closure](../build/component_publication.md#replacement-granularity).
No added target changes an unchanged uniform package layout, but quiescing calls
does not release stored borrows or code. Drain, retain the old era, or consume
and reconstruct packages around explicit state migration. A descriptor may not
reinterpret an old state layout through a new target.
