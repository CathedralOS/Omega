# Applied requirement indices

This is a focused Cathedral backing-lifecycle prerequisite, not a completed
nine-place fact row or a claim that Cathedral's backing lifecycle runs.

An explicitly applied boundary requirement preserves its complete type-index
tuple through helper arguments and results. Its outer trait telescope and its
member telescope are independent. Two calls to the same member do not share
result facts merely because they have the same declaration.

Run `omega run --both samples/cli/facts/applied_requirement_indices/main.omg`.
Both engines must exit 0 after observing payloads 29, 37, 43 and 47.
The comparisons check runtime payloads; they do not re-establish the static
index facts used by the calls.

The program exercises local forwarding, a projected field, another state,
nested results, a compound type argument and two distinct member tuples.
Build selects one named provider. The same program is pinned under
`tests/omega/pass/providers/applied_type_index_forwarding_exit`.
The adjacent fail fixtures `applied_type_index_wrong_actual`,
`applied_type_index_wrong_field`, `applied_type_index_nested_wrong_actual`
and `applied_type_index_wrong_result` reject mismatched argument indices
and a mismatched result guarantee.

`tests/omega/pass/providers/applied_routed_index_forwarding_exit` adds an
issuer-restricted qualification: the selected boundary issues a token carrying
`Issued<Word, Other>` or `Issued<Word, Word>`, and a generic helper forwards
each exact instance through two requirement calls. Both engines must observe
payloads 29 and 37. Its forged-constructor
and wrong-index controls reject. `applied_routed_parameter_mint` also rejects:
an ordinary call cannot introduce fresh parameter authority merely because its
requirement is an issuer. That introduction requires an installed external
root under `wiki/spec/resources/authority.md`; it is not a missing Unit lowering
feature.

The checked contract retains declaration-owned clause authority while applying
the selected telescope. Psi lowering must preserve the resulting memberships;
provider settlement must join each closed requirement to the selected
implementation's exact specialization. ABI planning, native emission and
Terminal execution must preserve the payloads. This sample does not establish
new allocation, borrowing, initialization, or cleanup authority.

Cathedral still needs the complete backed-storage journey: obtain granted vacant
storage, initialize a word to 17, let a helper write 29, read and move out 29,
and recover the exact vacant extent. This reduction isolates contract transport
needed by that journey. It does not cover scalar-qualified boundary values,
runtime-valued index binders, or every transport place.
