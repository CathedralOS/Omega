# Lifetimes and carried borrows

A lifetime relates a borrowed result or stored view to storage that remains
valid for its use. It does not create a loan, widen access, or identify a
referent by itself. [Loan resources](../terminal-psi/loans.md) retain the exact
captured place, lineage, formation and restoration evidence separately.

A `some` binder hides a type choice, not a reference lifetime. Packages keep
ordinary lifetime parameters or permitted elision; their fields and concrete
returned borrows remain tied to the same input/storage origins. Opening or
repacking cannot extend those lifetimes. See [some packages](some_packages.md).

## Binders and source applications

Lifetime binders use tick spelling in the declaration's `<>` list alongside
type, const and static-machine parameters. A reference places its lifetime
before its access modifier: `&'buffer T`, `&'buffer mut T`, or
`&'buffer write T`. Borrow-carrying data declares the lifetimes of its views:

```omega
data Message<'buffer> {
    body: &'buffer [u8];
}
```

Named data applications supply lifetime arguments before type/const/machine
arguments. They must match the declaration's lifetime arity and use binders
declared in the lexical owner. Duplicate binders and undeclared tags reject.
Lifetime arguments are erased regions, separate from runtime generic arity,
layout and monomorphization. Erasure does not remove semantic lifetime relations
or loan obligations. [Conformance telescopes](conformances.md#declaration-and-selection)
have their own normalized public mapping and application-elision rules.

Static callable requirements may additionally bind fresh lifetimes for each
invocation under [generic family matching](generics.md#invocation-lifetime-families).
These are distinct from captured-environment lifetimes. The rule permits repeated
reborrowing and requires universal requirement refinement; it does not introduce
general authored outlives bounds or higher-ranked whole-conformance arguments.

## Returned views

An explicit result lifetime that an input carries relates its reference leaves
to the corresponding input lifetime. One that no input carries is
[chosen by the caller](#caller-chosen-result-lifetimes). The source must supply
the result's declared access and remain live for the result's use. A signature
does not authorize returning unrelated storage merely because its type has the
same lifetime spelling.

An admitted foreign boundary may establish a provider-owned reference under its
complete safe contract, including through an output slot. Its backing need not
be contained in the lifetime-related input object: authority comes from
[provider admission](../build/foreign_storage.md#provider-owned-views-and-completion),
not from the lifetime spelling. Ordinary checked machines still establish
their result's backing and access.

For an elided output, one unambiguous reference input supplies the relation.
A qualified machine name, parameter name, or first-argument position grants no
priority among possible sources. Several unnamed carried
references do not become one source merely because they occupy one parameter.
Ambiguous elision requires an explicit relation, not a guess by name or position.
With no eligible reference input, the elided lifetime is one
[caller-chosen](#caller-chosen-result-lifetimes) lifetime. References inside
input aggregates count as inputs, and an input whose reference frontier is
unresolved, such as one of generic type, is not an input without references:
elision there requires an explicit relation.

Returned carriers preserve their complete structural lifetime frontier through
nested records, active sum payloads, fixed arrays, qualifications and concrete
generic arguments. A field's output path retains its corresponding source
paths and access; matching input/output field names are not required. Within a
selected input, every reference leaf carrying the selected lifetime remains a
possible source unless exact correspondence narrows it. Each possible source
must support the returned access. A direct reference returned from an owned
borrow-carrying input follows the same rule without an enclosing output path.
The result carries loans into the original backing, not a borrow of the caller's
private carrier storage.

Generic returned views require the complete instantiated frontier and its
result-to-input relation. A template-dependent frontier is not an empty one;
discarding the result cannot excuse missing call admission. General outlives
syntax and broader multiple-source result relations are not supplied by the
single-source elision rule. Unresolved loans cannot be omitted.

### Caller-chosen result lifetimes

Named decision: `image-storage-result-lifetime`, ratified 2026-11-13.

A lifetime that a machine binds and no input carries is chosen independently by
each caller: the call instantiates it from the result's use, as a quoted literal
takes the lifetime its use requires. The body must establish every returned view
for every admissible choice, including its backing, access and retention. It
cannot assume the choice coincides with an input lifetime, and checking never
shortens the promised lifetime to fit a local or input borrow. Instantiating a
lifetime creates neither backing nor retention authority.

```omega
machine package_name() -> &[u8] { "my_library" }

machine kind_name<'name>(value: &Value) -> &'name [u8] { "record" }

machine leak<'name>(text: &[u8]) -> &'name [u8] { text }   // Rejected.
```

`package_name` has no reference input, so its elided result lifetime is chosen
by the caller. `kind_name` binds one explicitly, so its result does not keep
`value` borrowed. `leak` rejects because `text` is valid only for its own input
lifetime. What qualifies is the backing a view denotes, not the form of the
returning expression: a literal, a subslice of one, a choice between such views,
and a view projected from another caller-chosen result all qualify. A view of
local storage does not, even when empty.

A [quoted literal](data_and_literals.md#quoted-bytes) satisfies every choice
because its image storage is retained for every accepted use. Where that image
can be reclaimed independently of the caller, the
[era custody rules](../build/component_publication.md#returned-values-and-custody)
decide whether the backing is retained: a view whose backing exists only while
one loaded image is held instead relates to an input that retains it, through
an ordinary lifetime parameter. There is no `'static`, image or component
lifetime constant.

A published signature retains whether each result lifetime is input-related or
caller-chosen. Forwarding, generic matching, requirement refinement and callable
construction preserve that distinction. A callable already instantiated at one
lifetime does not become one returning at any lifetime chosen later; this adds
no runtime lifetime-polymorphic callable values beyond the
[invocation-lifetime families](generics.md#invocation-lifetime-families). A
bodyless requirement or admitted boundary that promises a caller-chosen result
must establish backing for every choice under its contract.

## Carried-loan transport

Moving a borrow-carrying value transfers its exact contained loan paths and
access. Constructing an enclosing field or array element prefixes those paths;
it does not merge unrelated siblings. The leaves of one value formed together
are held at once, so they obey the compatibility of one call's operands: an
exclusive or write-only leaf must not overlap another leaf of that value.
Projecting a field retains that field's
loans. A fixed index can retain its element's loans, while an unresolved dynamic
selection conservatively retains all possible sources.

Assignment evaluates the replacement while old loans remain active, ends the
overwritten field's carried loans, then installs the replacement's exact loans.
If an owned move already emptied the field under a
[borrowed-storage invariant window](ownership.md#borrowed-storage-invariant-windows),
its contained loans moved with the removed value; filling the hole does not end
those loans a second time. The store installs the replacement's loans and closes
the exact place's restoration obligation, without ending the enclosing exclusive
loan prematurely.
Replacing one field neither preserves its former source nor releases a sibling's
loans. Domain-only casts, including removal of non-owning qualifications,
preserve source places and access. [Representation recasts](../layouts/recasts.md)
instead require the complete validated footprint and overlap judgment.

Transitions may transport references only while their storage outlives the
target path. Persistent fields retain exact field/case/index provenance across
control-flow edges only under valid source correspondence; a coincident name or runtime
index cannot supply it. Reference copies still denote live storage, unlike
integer snapshots. [Live-fact invalidation](state_contracts.md#mutation-and-subject-identity)
and loan authority remain separate checks.
