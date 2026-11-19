# C scalar-record layout through field

Library C layout preserves scalar values with C ABI field offsets, trailing padding and transparent alias identity.

Parent: [C-layout fact row](../README.md), `LIBRARY-REPRESENTATION-FAMILIES`.
The same 17/42/9 values, initialized independent backing, offsets 0/8/16,
24-byte extent, alignment 8 and transparent alias identity remain observable.
The receiving field carries the selected C representation. The twin changes
only that receiving place's policy to `OtherPolicy`, which delegates to the
same C geometry but remains a distinct application. Native and Terminal
execution both exit 0, and the twin rejects for its intended policy mismatch.
All four hosted entries and exact corpus mirrors exist. See the
[parent evidence](../README.md#transport-evidence).
