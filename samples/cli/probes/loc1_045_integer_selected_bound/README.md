# loc1_045_integer_selected_bound

The authored `>=` spelling is the operation identity: a selected integer
comparison emits the reversed `IntegerLessOrEqual` over the authored operand
pair, and the occurrence row records `Swapped` so the operation crash contract
reindexes the operator declaration's formal telescope into the emitted
positional roster. The body returns true, so the guarded transition takes its
first arm whether the run reads the selected provider or the emitted reversed
comparison.

Probe for the lane that collapsed the two selected-comparison occurrence
rosters (`LoweredSelectedIeeeFloatComparisonOccurrence` and
`LoweredSelectedIntegerComparisonOccurrence`) into the single
`LoweredSelectedComparisonOccurrence` row carrying `operand_order`.

The build binds all four hosted entries. After normal package review:

```sh
omega update --project samples/cli/probes/loc1_045_integer_selected_bound --target macos_arm64
omega run --both samples/cli/probes/loc1_045_integer_selected_bound/main.omg
```

The command above demonstrates native and Terminal execution on macOS ARM64;
actual native execution evidence is the host that ran it.