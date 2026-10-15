# Partitioned match records

Publish complete position/value records through runtime-selected borrowed output
partitions. Nonzero-offset windows retain their original backing and surrounding
sentinels; full and absent partitions reject insertion without modifying storage.
The record arrives both through a local and directly from a helper call.

Expected exit: 0 in native and Terminal execution.

```sh
omega update --project samples/cli/collections/partitioned_match_records --target macos_arm64 --offline --dangerously-accept-all
omega run --both samples/cli/collections/partitioned_match_records/main.omg
```

Both engines must follow each stored view to its original backing before
replacing the complete record. Indexing the descriptor as inline storage formerly
failed in Terminal's `StructuralReplacement` destination resolution while native
execution succeeded. This sample exercises caller-provided storage, not allocation
or the complete Squalr port.
