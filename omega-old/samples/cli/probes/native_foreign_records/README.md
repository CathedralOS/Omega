# Native foreign record results

Run on Windows x86-64 with `omega run samples/cli/systems/native_foreign_records/main.omg`.
Three calls compare UTF-16 `Omega` and `omega` ignoring case: a DLL import,
a dispatch-only table call, and the table call through a helper returning the
same record. Each must return comparison code `2`; any mismatch traps.

The five native arguments combine borrowed buffers, integer registers and an
outgoing stack slot. The one-field record result exercises ABI fragment capture
through allocation-inserted copies, independently of scalar-result handling.
This is a firmware-shaped transport witness, not a UEFI boot demonstration or
an interpreter provider. Native success is exit `0` after all three result checks.
