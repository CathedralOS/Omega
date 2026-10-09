# Nested case pattern guard meet

Prepare the package with `omega update --project .` and, if requested,
`omega update --project . --resume`, then run `omega run --both main.omg`.
Both engines must exit 0. The standalone Unit entrypoint has no library
dependency; an incorrect result crashes `Trap`.

An ordinary helper constructs an outer `Integer` carrying the nested `Hex`
case and a nonzero scalar count. Both selected cases are second declarations,
so this behavior cannot rely on first-case zero initialization. The structural
pattern tests outer membership, nested membership and later scalar payload
equalities. The nested test must preserve the enclosing case evidence needed
by those later tests and by the selected arm's `token.count` read.

The program also calls the same observer with the wrong outer case, the wrong
nested case and a different scalar payload. Each must select the wildcard and
return 1, not execute the matching arm. This follows ordinary pattern order and
exact-subject flow facts in `patterns.md` and `dependent_values.md`; it supplies
no new authority to read an inactive or mutated payload.

Existing invalid controls remain separate: `projected_float_payload_without_case`,
`projected_float_payload_after_guard_mutation`,
`projected_float_payload_after_negated_guard_mutation` and
`case_where_zero_refuted_first_case_rejected` must continue rejecting. The
`case_payload_after_effectful_constructor_guard` control also rejects a later
constructor whose child replaced the originally selected case.
