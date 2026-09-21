# SNAPSHOT-STORAGE — verify + record (z142)

Bare stub (TASKS.md:14480, unmarked; sibling stub
SNAPSHOT-STORAGE-AND-FILTERING at :14481). The name exists on the board
only as a claims-lane citation — `samples/apps/squalr` wholesale
dir-fences were recorded under "SNAPSHOT-STORAGE ×3 to ~05:2xZ"
(TASKS.md:7810 and again :14731) — so it mines the Squalr port's
snapshot storage surface: upstream `snapshot_region*` /
`snapshot_region_filter` storage machinery inside
`samples/apps/squalr`, part of the app board's GEOMETRY-PARITY
residual cluster (Omega-side row SQUALR-GEOMETRY-PARITY, TASKS.md:7757).

## Verified at `8f58b6676b` (linux x86-64)

- The implementing surface is `samples/apps/squalr` — a gitlink pinned
  `5b0307c352`, not checked out in this lane (no submodule objects).
- The surface is currently claimed wholesale: SQUALR-DEBUG-ASSERTIONS
  (Devin / z35, exp 2026-09-21T16:25Z) holds `samples/apps/squalr`.
- Earlier waves show the same shape: every citation of this name is a
  fence record on the app tree, and the enumerated gap rows
  (GEOMETRY-ALIGNMENT-STRING-PARSING, SQUALR-NAMED-TRAIT-OPERATORS,
  SQUALR-REGION-ALIGNMENT-EXPANSION, SQUALR-CLONE-SERIALIZATION-PARITY,
  SQUALR-DEBUG-ASSERTION-PARITY/-ASSERTIONS) each own their slice.

## Verdict

**Fenced / no unclaimed slice — record-only.** The name denotes the
port's snapshot-storage leg, which lives entirely inside the
wholesale-fenced submodule tree; implementation belongs to the lane
holding `samples/apps/squalr` (currently SQUALR-DEBUG-ASSERTIONS).
Coordinator: fold SNAPSHOT-STORAGE and SNAPSHOT-STORAGE-AND-FILTERING
into the SQUALR-GEOMETRY-PARITY residual family or the submodule
board's own snapshot row.
