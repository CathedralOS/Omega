# Owned claims survive an erased non-owning label

Parent: **BUMP-ALLOCATOR-CANARY** and the existing [resident-backing row](../README.md).

A granted vacant extent retains actual backing through transport,
initialization, permitted writes, and extraction.

The original allocator-backed local program is unchanged except for one
composition before partitioning: `mark` adds an independently routed label,
then `forget_label` casts to the bare carrier. The existing owned `Granted` and
`Vacant` claims must transfer with that exact extent. The separate `Marked`
provenance has no Content claim and must be erased. Neither equal geometry,
a return annotation nor the pending guarantee establishes an owned claim.

Both 32-byte windows still initialize the 24-byte record, observe the original
17, write 29, check the 101/103 neighbors and return their actual backing. The
positive binds all four hosted targets and requires native and Terminal exit 0.
This is a composition control in the existing fact, not another nine-place row.

The [invalid twin](invalid/main.omg) changes only `forget_label`'s guarantee to
also promise `Marked`. It must reject that erased label while the owned-claim
obligations remain valid. The original nine missing-vacancy twins, raw-child
qualification controls and non-owning Ticket-erasure controls remain required.
Actual checking and engine observations are pending.
