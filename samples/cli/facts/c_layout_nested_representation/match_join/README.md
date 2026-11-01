# Explicit nested C representation through match join

Library C layout preserves explicit child representation values, geometry and alias identity across transport.

Parent: [explicit nested representation row](../README.md), `LIBRARY-REPRESENTATION-FAMILIES`.
Both match arms deliver the complete representation; swapping the expected record and a decoy with first=18 makes selecting the wrong arm observable. Values 17, child 23/42/9 and tail 11, independent initialized backing, outer offsets 0/8/48, child offsets 0/16/32, extent 56 and alignment 8 remain observable. The twin changes only the child declaration to bare `Payload`; the C policy refuses default aggregate geometry.

Native/Terminal execution and intended refusal are pending. Exact corpus mirrors and all four hosted entry bindings exist.
