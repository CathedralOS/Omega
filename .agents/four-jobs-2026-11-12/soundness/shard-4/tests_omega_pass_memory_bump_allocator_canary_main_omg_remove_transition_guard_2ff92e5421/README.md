# Package allocator over real backing

`omega run --both tests/omega/pass/memory/bump_allocator_canary/main.omg`
must exit 0 in both Terminal Psi and native execution.

The program obtains 32 real bytes, opens the package Bump strategy, and allocates
two disjoint 16-byte extents. Both resident values coexist; writing one must not
change the other. An additional allocation fails without rewinding the cursor.
Extraction returns each value and its vacant extent, release retains both
returned claims, and checked `merge_vacant_extents` recomposes them. Reset joins
that complete prefix with the tail. Initializing and extracting 97 from the
restored backing makes reuse observable.

Only initial allocation, root issuance, vacancy establishment, the RAM profile,
and process exit cross admitted boundaries. Split, merge, allocation bookkeeping,
release, and reset are checked library bodies. Numeric geometry cannot replace
an outstanding linear claim or turn a live resident into vacant storage.

The `bump_allocator_reset_missing_prefix`,
`bump_allocator_reset_with_live_resident`, and
`bump_allocator_projected_claim_duplicate` rejection controls respectively omit
returned capacity, present live content, and transfer the same projected claim
twice. Existing allocator controls retain capacity and ownership checks.

Windows x86-64 observes native/Terminal exits 0/0. All four hosted target
bindings are authored; this run does not establish execution on the other hosts.
The parent BUMP-ALLOCATOR-CANARY task still owns genuine Vec growth and cleanup.
