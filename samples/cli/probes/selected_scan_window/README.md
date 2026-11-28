# Runtime-selected scan window

A helper replaces one descriptor selected by a runtime position. Both positions
are exercised: the caller observes the replacement's address and two-cell extent,
while the other descriptor still denotes its original one-cell backing. Expected
exit: 0.

```sh
omega packages update --project samples/cli/collections/selected_scan_window --target macos_arm64
omega run samples/cli/collections/selected_scan_window/main.omg
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/probes/selected_scan_window` from the repository root.

The runtime position does not grant static knowledge of the overwritten element.
Source checking retains every possible old and replacement backing until the
corresponding descriptors and their children are no longer live.
