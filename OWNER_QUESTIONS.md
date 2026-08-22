# Owner Questions

Only unresolved owner-level language or architecture decisions belong here.
Settled decisions live in the language guide and design briefs; implementation
and deliberately deferred research live in `TASKS.md`. Questions are numbered
consecutively; pruning or adding one requires updating every repository
reference in the same change.

Question numbers are mutable queue positions, not permanent decision identities.
Code, canaries, and settled documentation must cite a stable named decision or
the governing guide section rather than an owner-question number. A settled
decision's durable identity does not change when this queue is pruned.

Last pruned: 2026-08-22.

## Q1 — How does a target package declare a nominal foreign endpoint?

The foreign-binding model requires source to cite one namespace-owned
`DllImportId`, for example `Windows::Kernel32::WriteFile`, while raw library and
export bytes live only in sealed, fingerprinted target/link metadata. The
repository does not yet define the declaration that creates that nominal value
or the authored target input that maps it to those raw bytes. An ordinary
`const` cannot construct the opaque ID without reopening free pairing, and
deriving the ID from either strings or the realization machine would contradict
the settled identity rule.

Choose the target-package declaration and metadata-supply surface. It must:

- create one resolved nominal symbol usable as a `DllImportId` expression;
- bind that symbol inseparably to one library/export pair in sealed target/link
  metadata, with no raw strings in ordinary Omega source;
- make ownership, target applicability, duplicate/missing mapping rejection,
  fingerprinting, and package visibility explicit; and
- generalize coherently to `CallingPlanId`, firmware/table IDs, and other
  mechanism-specific nominal values without inventing a string-backed escape.

Recommended direction: a target-package-owned nominal-ID declaration plus a
separate sealed target metadata record keyed by that resolved declaration. Keep
`build.omg` limited to selecting target/provider declarations; it must neither
author linker spellings nor manufacture IDs.
