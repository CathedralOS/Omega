# Library-owned vector growth

Parent: **BUMP-ALLOCATOR-CANARY**.

A granted vacant extent retains actual backing through transport,
initialization, permitted writes, and extraction.

This is the package consumer of the existing [runtime-growth production](../runtime_growth/README.md),
not another fact or a larger count. The ordinary alloc package owns the vector
operations; the application supplies backing and selects its resource profile.
The same library has no capacity-specific operations or application-owned pool.

The program allocates two buffers, grows from capacity 2 to 5, checks the
distinct prefix values 17 and 29 and the appended 43, returns both buffers,
resets the allocator, and reuses all 48 bytes for a vector containing 97.
Those reads also check that copying one cell does not overwrite its neighbors.
The exact returned base and length contracts prove recomposition without a
post-call equality guard. Allocation and access profiles remain admitted inputs;
no vector operation grants memory authority.

Run `omega run --both main.omg`; both engines must exit 0.
All four hosted entry/provider selections are declared; execution evidence
must identify the host actually run.

The [invalid twin](invalid/main.omg) removes only vacancy establishment.
It must reject the unestablished qualification, not fail to resolve a library
symbol. The corpus also retains live-loan and double-cleanup controls against
the real library under `fail/collections/vec_*`.

The element type here is u32. General non-copy elements need resident custody
and observable element cleanup; copy-scalar bytes do not establish that support.
