# Zero-sized owned element

Parent: `EXTENT-LIBRARY-TRANSFORMS` / generic Vec.

An empty record with a cleanup hook still owns a disposition obligation. This
program initializes it into a zero-byte singleton Run, extracts it, and drops
it exactly once after printing `M`. Both engines must exit zero and print
exactly `MD` plus a newline. An empty Run with the same byte geometry travels
alongside it and is released without running cleanup.

The [reject twin](../../../fail/layouts/run_zero_sized_double_drop/main.omg)
adds only a second drop of the extracted value. Empty geometry must not permit
duplicate consumption. This covers singleton storage, not multi-cell Run
partitioning, generic Vec growth, or allocator return.
