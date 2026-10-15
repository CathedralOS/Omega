# Reborrowed partition output

Append complete scan records through a local borrowed view of caller-provided
partitions. Capacity guards precede the reborrow; the local view must preserve
the indexed partition's count, nested output length and original backing.
Nonzero-offset outputs retain surrounding sentinels, and full or absent
partitions decline insertion. Expected native and Terminal exit: 0.

```sh
omega update --project samples/cli/collections/reborrowed_partition_output --target macos_arm64 --offline --dangerously-accept-all
omega run --both samples/cli/collections/reborrowed_partition_output/main.omg
```

This exercises live bounds and exact count increments through a full-window
reborrow. Writes to record contents preserve the sibling count and descriptor
length, while replacing either observation invalidates its old facts. Descriptor
cleanup ends the local carrier without disposing the caller's output storage.
This is not a substitute allocator or complete Squalr scan engine.
