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
omega packages update --project samples/cli/probes/loc1_045_integer_selected_bound --target macos_arm64
omega run samples/cli/probes/loc1_045_integer_selected_bound/main.omg
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/probes/loc1_045_integer_selected_bound` from the repository root.

Earlier macOS ARM64 qualification covered both interpretation and the published executable.