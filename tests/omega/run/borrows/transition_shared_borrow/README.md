# Shared element reference at a transition tail

Run `omega run --both tests/omega/run/borrows/transition_shared_borrow/main.omg`.
Both engines must exit 0. The returned source must retain its exact field and
fixed-index coordinates through ordinary transition completion. Discarding the
pure call does not exempt its machine from reference-result validation.

`pass/borrows/runtime_state_fixed_element_reference_exit` additionally observes
the returned referent after a named-state hop at a nonzero array index.
