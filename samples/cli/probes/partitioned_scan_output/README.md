# Partitioned scan output

Scan positive values into a runtime-length collection of caller-supplied borrowed
partitions. Empty and full partitions are skipped without flattening storage;
when capacity ends, the returned cursor identifies the first unconsumed input.
Repeated scans of full output, empty input and empty output preserve contents.
Nonzero-offset windows and surrounding sentinels pin the original backing.

Expected exit: 0 in native and Terminal execution. This is a nested collection
write capability, not an allocator implementation or complete Squalr port.

```sh
omega update --project samples/cli/collections/partitioned_scan_output --target macos_arm64 --offline --dangerously-accept-all
omega run --both samples/cli/collections/partitioned_scan_output/main.omg
```

The stored count's exact increment follows from
`partitions[selected].count < partitions[selected].values.len`. Writing one
element preserves that descriptor's extent and the containing record's sibling
count; it does not preserve facts after replacing the descriptor or changing a
selector. The same operation must work through nested descriptors and local
scalar captures without flattening storage or re-evaluating captured indexes.
