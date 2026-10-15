# Repeated partition scan

Compact complete position/value records in a borrowed partition, retaining
their original order. Repeated filters reduce four records to two, then one,
then none; filtering an empty result stays empty. Nonzero-offset backing retains
both surrounding sentinels. Expected native and Terminal exit: 0.

```sh
omega update --project samples/cli/collections/repeated_partition_scan --target macos_arm64 --offline --dangerously-accept-all
omega run --both samples/cli/collections/repeated_partition_scan/main.omg
```

This exercises repeat filtering over caller-provided storage, not allocation
or the complete Squalr application. The command above runs with native exit 0
and Terminal exit 0 on macOS ARM64. Bounds survive element mutation without
confusing the slice descriptor with its contents; replacing a descriptor or
its scalar bound still invalidates the affected facts.
