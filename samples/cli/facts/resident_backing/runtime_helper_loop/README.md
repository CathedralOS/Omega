# Allocation custody through a nested helper loop

Parent: **BUMP-ALLOCATOR-CANARY / VEC-NATIVE-GROWTH**.

A granted vacant extent retains actual backing through transport,
initialization, permitted writes, and extraction.

This is another production within the existing resident-backing row, not a
new fact or a larger-count variant. `runtime_loop` forwards descriptors directly;
here a nested machine calls identity helpers and forwards their results on the
backedge. The checker must derive each result's input correspondence rather than
mistake its temporary receiving home for a fresh allocation.

`main.omg` obtains 48 real zeroed bytes, splits them into disjoint 16- and 32-byte
regions, exercises zero and repeated iterations, and reads/writes through ordinary
access views. Distinct values on the two regions detect swapped or aliased
results; subsequent writes verify that neither region overwrote the other.
The static obligation is continued `Granted & Vacant` custody of both regions.
Access establishment checks the runtime view footprint; this witness does not
claim to prove a recursive descriptor-equality postcondition.

`invalid/main.omg` changes only the second helper's argument: it attempts to
transfer the first region again after moving it. It must reject the duplicate
transfer. Both programs bind all four hosted targets. Run the positive with
`omega run --both main.omg`; other-host execution needs a matching host.

Vec growth remains open: its copy operation also transforms and rejoins backing
claims, preserves descriptor geometry through recursive contracts, and must
release/reset the allocator. This witness does not replace that acceptance.
