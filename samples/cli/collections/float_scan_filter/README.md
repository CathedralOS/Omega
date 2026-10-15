# Floating scan repeat filter

Read little-endian binary64 snapshots through borrowed byte slices, select a
comparison implementation at runtime, and compact matching address records in
place. The second scan keeps only increased values; the third removes them.
Signed zeros compare equal, while distinct NaNs are unequal and unordered.
The address signature checks stable ordering and the outer records are sentinels.
The signature uses explicitly wrapping arithmetic; it is not an exact sum claim.

Expected native and Terminal exit: `0`, with no program output.

```sh
omega update --project samples/cli/collections/float_scan_filter --target macos_arm64 --offline --dangerously-accept-all
omega run --both samples/cli/collections/float_scan_filter/main.omg
```

The byte views retain the borrowed snapshot's actual extent. Proving eight
available bytes authorizes one binary64 read; it does not replace the descriptor
length or establish additional value predicates.
