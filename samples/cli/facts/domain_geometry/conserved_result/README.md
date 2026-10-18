# Conserved qualified result

Parent: **BUMP-ALLOCATOR-CANARY**. An admitted no-wrap domain predicate proves
`base + requested` fits `addr` when `requested < length`. The split returns
two qualified linear values whose interval content conserves the input.

Run `omega run --both samples/cli/facts/domain_geometry/conserved_result/main.omg`
and the same command with `projected/main.omg`. Both expect exit 0; the second
moves the children through fields of an intermediate record before qualifying
them. Neither program owns backing or establishes storage authority.

`reference_carrier/main.omg` and `reference_carrier/field/main.omg` exercise
the same owned qualification operation on a carrier containing a reference.
Both expect exit 0 after reading 42 through the transferred reference; the
second moves the carrier into an anonymous call-argument record and then out
through its field. Qualification must preserve the original reference and
retire the source once, not copy its custody or release it again.
`reference_carrier/returned/main.omg` returns the qualified carrier through
the call result before reading the same 42; it also expects exit 0.
`reference_carrier/selected/main.omg` exercises both runtime choices through
an anonymous argument: the same source with and without explicit qualification,
and distinct sources with references to 42 and 43. Every run expects exit 0.
The skipped source keeps its cleanup obligation; selection cannot authorize
either a second use of a moved source or dropping an unmet cleanup prerequisite.

The matching pass fixtures live in `tests/omega/pass/facts/` under
`domain_geometry_conserved_result_exit` and
`domain_geometry_projected_conserved_result_exit`. The wrong-partition control
must reject its conservation guarantee; `qualified_linear_value_reused_after_transfer`
must reject reuse of the consumed owner. The `entry_domain_return_*` controls
check that writes and unrelated state bindings cannot supply entry predicates.
`owned_qualified_reference_carrier_exit` and
`owned_qualified_reference_field_exit` pin the reference compositions; their
matching `*_reuse` controls reject an observation after the owner moved.
`owned_qualified_reference_result_exit` and its reuse control pin the
structural-result call with an anonymously constructed argument.
`owned_qualified_reference_selection_exit` and its reuse control pin the
selected argument. `drops/selected_argument_retains_cleanup_prerequisite`
rejects disposal of the skipped owner when its hook requirement is false.

These are return-proof and owned-qualification witnesses, not completion of the
nine-place domain-geometry row or the backed allocator. The real Extent customer
still requires authorized child grants, conserved parent custody and backed
allocation, coexisting residents and reset.
