# C scalar-record layout through target body

Library C layout preserves scalar values with C ABI field offsets, trailing padding and transparent alias identity.

Parent: [C-layout fact row](../README.md), `LIBRARY-REPRESENTATION-FAMILIES`.
Build selects ordinary named implementations for four target entries. A separate 17/29 marker proves which selected implementation supplied the same C record. Values 17/42/9, initialized independent backing, offsets 0/8/16, extent 24, alignment 8 and alias/explicit identity remain observable. The twin changes one receiving type to a distinct policy with identical geometry.

Native/Terminal execution and intended refusal are pending. Exact corpus mirrors and all four hosted entry bindings exist.
