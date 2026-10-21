# Returned placed access

`omega run --both main.omg` must exit 0 in both engines. An ordinary helper
forms a shared placed reference to the original two-word RAM object and returns
it inside a case. The caller reads element 1 through its External accessor and
passes that result to a state requiring `u64`; the observed value must be 41.

This pins two required joins: named-state receiving context for generic call
results, and original backing provenance through a returned placed carrier.
The empty `NoReturnedStorage` record carries no resource on rejection; it is
not Unit-as-storage and does not discharge a loan by itself.

The exact-value-type negative is
`fail/layouts/placed_transition_result_type_mismatch`. Existing
`fail/layouts/placed_reference_results/{wrong_backing,overlapping_loans}`
retain the separate lifetime and aliasing boundaries. This does not establish
external Extent qualification, provider admission, or Cathedral CR3 switching.

`fail/layouts/placed_return_alternative_wrong_lifetime` also rejects a
returning state whose backing has a different lifetime from the result.
This fixture establishes single-backing transport, not selection among
multiple backing roots for one result leaf.
