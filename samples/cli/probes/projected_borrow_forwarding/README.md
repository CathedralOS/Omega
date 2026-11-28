# Projected borrow forwarding

Expected exit: 0. A returned mutable field borrow must retain its captured
indexed element while unrelated cursor and marker fields change. The same
operation forwards through nested calls and a nested machine. A returned array of
disjoint borrows must preserve each element's independent permission.

After ordinary package review, run:

```sh
omega run samples/cli/collections/projected_borrow_forwarding/main.omg
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/probes/projected_borrow_forwarding` from the repository root.

The native and Terminal runs both return 0 on macOS ARM64. The regression
fixtures also put extraction directly in a stateful entry, transfer nested rows
in reverse order, and leave an unselected descriptor for residual cleanup.
