# Repeated partition scan

Compact complete position/value records in a borrowed partition, retaining
their original order. Repeated filters reduce four records to two, then one,
then none; filtering an empty result stays empty. Nonzero-offset backing retains
both surrounding sentinels. Expected native and Terminal exit: 0.

```sh
omega packages update --project samples/cli/collections/repeated_partition_scan --target macos_arm64 --offline --dangerously-accept-all
omega run samples/cli/collections/repeated_partition_scan/main.omg
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/probes/repeated_partition_scan` from the repository root.

This exercises repeat filtering over caller-provided storage, not allocation
or the complete Squalr application. The earlier macOS ARM64 comparison recorded exit 0
for both the published executable and the interpreter. Bounds survive element mutation without
confusing the slice descriptor with its contents; replacing a descriptor or
its scalar bound still invalidates the affected facts.
