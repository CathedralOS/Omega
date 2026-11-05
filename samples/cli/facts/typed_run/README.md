# Dense runtime-count ownership

Parent: **BUMP-ALLOCATOR-CANARY**.

Fact: **A live run retains the actual established cells and their obligations through checked custody transfers.**

The nearest existing row is [resident_backing](../resident_backing/README.md).
That row owns one complete resident value; this row owns an ordered runtime
number of cells with their individual obligations retained opaquely. Its controls
must reject forged count/custody, invalid cell partitions and duplicate extraction,
not merely invalid byte geometry or wrong resident policy identity.

The [singleton rung](../../../../tests/omega/pass/facts/typed_run_singleton/main.omg) first establishes an owned Word in
real allocated storage and extracts it. All three scalar fields are checked;
the cleanup hook must print exactly once after the explicit `M` marker, giving
`MD` plus a newline. Both engines must exit 0. This rung does not complete the
runtime-count fact or generic Vec.

Command: `omega run --both tests/omega/pass/facts/typed_run_singleton/main.omg`.
The [rejected-initialization rung](../../../../tests/omega/pass/facts/typed_run_singleton_rejection/main.omg) supplies an
oversized extent, checks `IncompatibleSupply`, and recovers the original extent
and value with the same exactly-once cleanup oracle. The [count-mutation
control](../../../../tests/omega/fail/facts/typed_run_singleton_count/main.omg) changes an established run's count
before extraction and must reject its invalidated `Live` qualification.
The [double-drop control](../../../../tests/omega/fail/facts/typed_run_singleton_double_drop/main.omg) must reject reuse of the
extracted value. These witnesses are retained under
`tests/omega/{pass,fail}/facts/typed_run_*` alongside nested-owner transfer
controls for borrowed, duplicate and ambiguous input custody.

Both singleton paths capture the admitted backing's base and length before
initialization. Their following machines require those same coordinates from
the returned storage, so a runtime equality guard cannot substitute for the
operation preserving its static contract. The
[false-geometry control](../../../../tests/omega/fail/memory/typed_run_backing_geometry/main.omg)
instead demands a different length and must reject at the call to `extract`.

The optimized fixed-base producer at `7e13cc9e1d505` rejects at import resolution:
`failed to resolve .../source/library/core/run.omg` (file not found). This was
measured before compiler changes. All four hosted targets are bound, but only
actual host executions count as evidence.

The current working implementation executes the unchanged singleton on Windows
x86-64 with native and Terminal exit 0 and output `MD\n`. Its double-drop control
rejects use after transfer. A count-changing helper cannot promise `Live` on
return, and a changed owned descriptor cannot return as `Run in Live`. An
entry-only qualification may expire; accepting such a helper alone is not
evidence that it preserved live custody. Runtime-count transformations remain
open; this singleton does not establish their acceptance.

Remaining acceptance: all nine transport places and fact-sensitive reject twins;
ordered split/merge, relocation, failure accounting, zero-sized obligations,
contained loans/carry anchors, native slices and domain-preserving mutable views;
and the generic library Vec consumer. Retained Terminal evidence must carry the
checked cell transformation into both engines without per-element runtime tags,
custody tables or hidden occupancy updates. No part of that acceptance is closed
by the presence of this first witness.
