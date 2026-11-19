# C scalar-record layout through other state

Library C layout preserves scalar values with C ABI field offsets, trailing padding and transparent alias identity.

Parent: [C-layout fact row](../README.md), `LIBRARY-REPRESENTATION-FAMILIES`.
The complete C record crosses an explicit terminal call into a nested machine; nesting captures no runtime locals. Values 17/42/9, initialized independent backing, offsets 0/8/16, extent 24, alignment 8 and alias/explicit identity remain observable. The twin changes one receiving type to a distinct policy with identical geometry.

Linux x86_64 native and Terminal execution exit 0; the one-type policy twin
rejects as intended in [pilot 41303478559](https://github.com/CathedralOS/Omega/actions/runs/41303478559).
The exact corpus mirrors independently pass checking and execution. Checking
records its actual default `macos_arm64` target; other hosts were not executed.
All four hosted entry bindings exist. Producer identity and measurements are
recorded in the [row evidence](../README.md#transport-evidence).
