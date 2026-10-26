Run `omega run --both tests/omega/pass/drops/runtime_selected_partial_owner_cleanup_exit/main.omg`. Conditional record-field and fixed-array-element transfers keep their residual owners until normal death. Replacing a conditionally moved field or whole owner disposes the old value only on the path retaining it, before the new store. Both modes must match the authored hook order and exit 0, even after overwriting the selection condition.

The record is generic; fixed-array replacement preserves the same old/new
occurrence boundary and reverse index cleanup order.

The pure linear pair checks that replacing a possibly moved field retires its
old claim before establishing the replacement, while retaining the sibling.
Separate selected fields and replacement of their containing record exercise
ordered residual cleanup without an auxiliary owner or runtime liveness flag.

The fixture explicitly assumes finite normal completion of its reviewed byte-write
provider and binds real libc/MSVCRT imports. It does not strengthen the standard
console contract or depend on the corpus harness supplying a custom intrinsic.

An optional linear payload exercises both empty and held cases with selection
enabled and skipped. The payload retains a linear receipt and
an affine token whose hook prints once, before the younger owner when consumed
and after it when retained until normal death.

Cleanup selects the optional case before disposing either payload sibling;
it must not read the discriminator again after one sibling has been released.
Each selected receiver keeps its own availability check, and an absent sum
skips the complete case dispatch. A partially moved sum does not authorize a
new discriminator read or whole-owner cleanup.
