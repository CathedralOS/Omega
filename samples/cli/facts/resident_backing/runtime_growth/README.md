# Preserve elements while growing real backing

Parent: **BUMP-ALLOCATOR-CANARY / VEC-NATIVE-GROWTH**.

A granted vacant extent retains actual backing through transport,
initialization, permitted writes, and extraction.

This production uses the ordinary Bump package to obtain two coexisting
allocations. It fills the first, copies its live prefix to the larger one
through a terminating nested helper, appends another element, and reads all
three values back. It then releases both allocations, recomposes their backing,
resets the allocator, and reuses all 48 bytes for a new vector containing 97.

The copy loop returns both backing descriptors with their original base and
length. Those guarantees establish the reconstructed vector's invariant
directly; a caller guard does not replace them. This is the allocator customer
of the reconstruction production, not another fact row or a size variation.
It composes runtime split/merge, read/write views, recursive result contracts,
and real allocation rather than adding a new compiler operation for vectors.

The local vector operations expose the compiler path. They are not the
completed generic library Vec migration. The [library consumer](../library_growth/README.md)
uses the reusable u32 implementation; generic element cleanup remains open.

Allocation publishes the nonempty extent's position against the allocator's
original capacity. A mutable allocation changes the remaining count and tail,
not the base or capacity. Those unchanged field observations must survive the
call so the returned position proves adjacency for release and reset. Neither
a post-call guard nor a new storage assumption supplies that correspondence.

Run `omega run --both main.omg`. Both engines must exit 0 after observing
17, 29, and 43, length 3 and capacity 5, followed by 97 in the reused backing.
Different values make a skipped copy,
wrong source, wrong offset or overwritten neighbor observable. Four hosted
entry/provider selections are authored; execution evidence is host-specific.

`invalid/main.omg` changes only the recursive copy's two backing arguments.
It must reject the unchanged original/replacement descriptor guarantees;
descent does not excuse incorrect argument substitution or grant new custody.
