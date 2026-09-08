# Lifetimes and carried borrows

A lifetime relates a borrowed result or stored view to storage that remains
valid for its use. It does not create a loan, widen access, or identify a
referent by itself. [Loan resources](../terminal-psi/loans.md) retain the exact
captured place, lineage, formation and restoration evidence separately.

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

## Returned views

An explicit result lifetime relates its reference leaves to the corresponding
input lifetime. The source must supply the result's declared access and remain
live for the result's use. A signature does not authorize returning unrelated
storage merely because its type has the same lifetime spelling.

For an elided output, a borrowed receiver supplies the usual receiver relation;
otherwise one unambiguous reference input supplies it. Several unnamed carried
references do not become one source merely because they occupy one parameter.
Ambiguous elision requires an explicit relation, not a guess by name or position.

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
single-source elision rule. Current source-checker restrictions are recorded
[beside checking](../../../omega-rust/psi/pipeline/typed-trees-to-checked-trees/README.md#lifetime-source-correspondence),
not additional permissions to omit unresolved loans.

## Carried-loan transport

Moving a borrow-carrying value transfers its exact contained loan paths and
access. Constructing an enclosing field or array element prefixes those paths;
it does not merge unrelated siblings. Projecting a field retains that field's
loans. A fixed index can retain its element's loans, while an unresolved dynamic
selection conservatively retains all possible sources.

Assignment evaluates the replacement while old loans remain active, ends the
overwritten field's carried loans, then installs the replacement's exact loans.
Replacing one field neither preserves its former source nor releases a sibling's
loans. Same-carrier casts and explicit erasure of non-owning qualifications
preserve source places and access. [Representation recasts](../layouts/recasts.md)
instead require the complete validated footprint and overlap judgment.

Transitions may transport references only while their storage outlives the
target path. Persistent fields retain exact field/case/index provenance across
states only under valid source correspondence; a coincident name or runtime
index cannot supply it. Reference copies still denote live storage, unlike
integer snapshots. [Live-fact invalidation](state_contracts.md#mutation-and-subject-identity)
and loan authority remain separate checks.
