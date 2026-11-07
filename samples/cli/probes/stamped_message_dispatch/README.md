# Stamped message dispatch

A message retains common metadata whether it is missing or carries a borrowed
cell. Dispatch observes only the active payload. Its caller then retires the
borrow, preserves the stamp, and writes the backing cell.

Expected exit: 0. The received message yields value 19 and stamp 73; the missing
message yields value 0 and stamp 41. Both paths observe the later backing write
of 23.

Run from the repository root:

```sh
omega run --both samples/cli/collections/stamped_message_dispatch/main.omg
```

Both authored branches run through native and Terminal execution. Case
selection preserves the common metadata and selected payload without extending
the backing loan past the last use.
