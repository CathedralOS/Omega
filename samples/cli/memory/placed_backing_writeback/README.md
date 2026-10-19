# Placed access to original RAM

Expected exit: 0 from `omega run --both main.omg` after ordinary package
preparation. The program borrows initialized RAM through `Placed::view_mut`,
writes a nonzero-offset accessor in a helper state, reads another accessor,
then observes the original RAM after the loan ends. A returned reference selects
between two distinct backings; both choices execute, and the unselected backing
must remain unchanged.

This is a Cathedral allocator prerequisite: actual backed access, not numeric
interval simulation. It does not establish an `Extent`, initialize a resident,
allocate storage, or prove reset and return. Those remain the parent
`BUMP-ALLOCATOR-CANARY` acceptance.

The native operation consumes the current accessor reference, whose projection
already includes the field's container offset. It must neither recover a former
parent pointer nor add that offset twice. Placement/provider admission and live
reference custody remain distinct checks. The regression fixture is
`tests/omega/pass/layouts/runtime_placed_accessor_writeback_exit`; conflicting
parent access and shared-to-mutable controls remain under `fail/layouts`.
