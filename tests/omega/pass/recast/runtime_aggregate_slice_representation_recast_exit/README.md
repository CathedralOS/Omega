# Typed aggregate slice representation views

Run the authored program with a reviewed host package lock:

```text
omega run --both tests/omega/pass/recast/runtime_aggregate_slice_representation_recast_exit/main.omg
```

Both engines must exit 70. The shared `Narrow` to `Wide` view weakens the
integer range; the mutable `Narrow` to `Exact` view preserves both directions
of validity. Record field names differ, and a Boolean leaves one padding byte
in each four-byte element. Reads and writes therefore retain field correspondence
and element stride rather than treating records as densely packed scalars.

The mutable view retires on the edge into `verify` before the original array
is observed. Its updated signed integer and Boolean fields must be visible in
that original storage; constructing a detached target array cannot satisfy it.

Before this implementation, package candidate checking omitted the entry unit
at stage 04 `state graph: terminator: conditional successors: parameter transfer`.
The execution check is the authored result above, not merely advancing that
construction boundary.
