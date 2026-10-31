These source machines qualify ordinary call custody through the public Terminal
embedding API in `compiler::placed_view_calls`. The host explicitly lends each
original backing and binds only the exact generated read/write imports.

Frozen results are status `41` for shared, mutable, nested, Unit-returning and later-state calls;
mutable calls write `9` to the original backing. Reordering distinct backings
with statuses `41` and `73` returns `73041`. Mixed scalar/ordinary/placed inputs
return `7 + 12 + 41 = 60`, joining source parameter 2 to structural parameter 1.
The test checks effects and backing contents, successful loan retirement,
missing custody, exclusive overlap, conflicting shared descriptor associations,
and modified runtime/state/carrier coordinates. Shared aliases of the same exact
backing remain admissible.

This package binds no native program entry. Native subordinate establishment
still refuses at `AccessViewInputsRequireCustodyLowering`; the test preserves
that authority gate. The host adapter is a target-neutral external operation
model, not a claim of physical native placement or device authority.
