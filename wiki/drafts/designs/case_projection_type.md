# Case projection type

Ratified 2026-11-09. A pattern can split mixed data into its common fields and
one case-only view, but that view had no type an author could write. A helper
that wanted only the cases had to take the complete value and decompose it
again. `Command::case` now names the view's type, so the helper's signature
can say exactly what it accepts:

```omega
machine targets_one_user(operation: &Command::case) -> bool {
    match operation {
        Command::GetUser { user_id as _ } -> true
        Command::CountUsers -> false
    }
}
```

The normative owner is the [case projection type](../../spec/language/patterns.md#case-projection-type).
Track implementation under `CASE-TYPE` in [TASKS.md](../../../TASKS.md). Delete
this record when the implementation and acceptance are complete.

## Decisions

- `Data::case`, or `Data<T>::case` for generic data, names the component that
  `case as name` binds. Decomposing `&Data` binds it at `&Data::case` and
  decomposing `&mut Data` at `&mut Data::case`.
- The name adds nothing: no case owner, tag, storage slot, allocation, copy or
  layout. Cases keep their paths. Two declarations with identical cases have
  distinct projection types.
- The type is written only behind a reference. It has no literal, zero default
  or cast. `Data::case` rejects for data with no common fields and for a
  record with no cases.
- The reference keeps the original storage identity, lifetime, access and
  loans, and reaches only the case component. Common fields stay disjoint
  sibling places.
- A helper is checked against its own contract. The type guarantees payload
  field types. A data `requires` and the caller's facts about the complete
  value do not cross the call unless the helper requires them.
- Writes through `&mut Data::case` leave the lender owing the complete value's
  predicates at handback.

This is not a second way to declare a sum. When the cases have an identity and
lifecycle of their own, declare them as their own data and store that as a
field.

## Implementation order

1. Resolve `Path::case` in type position to the existing case-view type and
   reject it for pure sums, case-free records and by-value positions.
2. Admit a decomposition binding as an argument at `&Data::case` and
   `&mut Data::case`, carrying the subject's loans.
3. Check the callee independently: case coverage and payload field types from
   the projection, nothing from the enclosing value.
4. Apply contract-based repair to the lender after a mutable projection call.

## Fixtures and acceptance

- A helper taking `&Data::case` matches every case and is called from a
  decomposition; the same helper missing an arm rejects.
- A caller keeps a shared borrow of a common field across a call that takes
  `&mut Data::case`.
- A helper relying on a data `requires` it did not state rejects; stating it
  and proving it at the call passes.
- `Sum::case` on a pure sum and a by-value `Data::case` parameter each reject.

Acceptance: these pass `omega run --both`.

## Left open

- Whether an owned case component can cross a call, and what it owns when the
  common fields are dropped separately.
- Whether a projection type may instantiate a generic parameter or be a
  conformance subject.
- Reflection over a projection type.
