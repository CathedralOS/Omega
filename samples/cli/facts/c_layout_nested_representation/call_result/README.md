# Explicit nested C representation through call result

Library C layout preserves explicit child representation values, geometry and alias identity across transport.

Parent: [explicit nested representation row](../README.md), `LIBRARY-REPRESENTATION-FAMILIES`.
A machine returns the complete selected child and outer C record before alias/explicit application interchange. Values 17, child 23/42/9 and tail 11, independent initialized backing, outer offsets 0/8/48, child offsets 0/16/32, extent 56 and alignment 8 remain observable. The twin changes only the child declaration to bare `Payload`; the C policy refuses default aggregate geometry.

Linux x86_64 native and Terminal execution exit 0; the one-declaration default-child twin
rejects as intended in [pilot 41311782126](https://github.com/CathedralOS/Omega/actions/runs/41311782126).
The exact corpus mirrors independently pass checking and execution. Checking
retains its actual default `macos_arm64` target; other hosts were not executed.
All four hosted entry bindings exist. See the [row evidence](../README.md#transport-evidence)
for producer identity and measurements.
