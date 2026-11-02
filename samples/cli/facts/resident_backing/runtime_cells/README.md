# Runtime cell backing

Parent: **BUMP-ALLOCATOR-CANARY / VEC-NATIVE-GROWTH**. This is a runtime-partition production of the existing `resident_backing` fact, not another size family.

The same 48-byte allocation is split at a runtime offset, lends a four-byte cell through a qualification-erasing mutable access view, and is rejoined. The checked result promises the original base and length. Both zero and nonzero prefixes execute; repeated writes and an adjacent untouched cell distinguish real backing from detached storage. The twin permits an offset without room for the cell and must reject the second split.

The boundary supplies backing and a stable-RAM profile, not scalar facts or partition authority. Content writes preserve descriptor geometry and existing vacancy while invalidating old content observations. The completed storage graph distinguishes writes to inline cells from observations through the descriptor; call completion retains the descriptor only when that exact call preserves it. Unknown content effects remain unknown. The split/merge postconditions remain ordinary checked library contracts.

Run `omega run --both samples/cli/facts/resident_backing/runtime_cells/main.omg`; check `invalid/main.omg`. Both engines return 0 on Windows x86-64; the missing-bound twin rejects at the second split. This prerequisite does not close Vec growth/reallocation/drop.

[Plain RAM](plain_ram/main.omg) checks the converse: a qualification-erasing
loan to an ordinary record updates that record, while preserving its untouched
neighbor. Its [invalid twin](plain_ram/invalid/main.omg) claims the old value
survives the write and must reject that postcondition.
