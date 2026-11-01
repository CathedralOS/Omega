# Explicit nested C representation

Fact: library C layout preserves explicit child representation values, geometry and alias identity across transport.

Parent: `LIBRARY-REPRESENTATION-FAMILIES` in [TASKS.md](../../../../TASKS.md).

The nearest existing row is [C scalar-record layout](../c_layout_scalar_record/README.md).
That row establishes schema-driven scalar placement. This row adds composition
of an explicitly selected child representation, whose 40-byte extent differs
from the semantic child's 24-byte default storage. The outer C record must use
child offset 8, tail offset 48, complete extent 56 and alignment 8. Independently
initialized `[u64; 7]` backing checks the selected placements without reading
uninitialized padding. The scalar values are 17, child 23/42/9, and 11.

The initial local witness is being qualified; no passing execution is claimed.
The complete nine transport places and default-child rejection twins remain open.
The generic schema metadata must distinguish selected child geometry from native
aggregate geometry without granting storage or access authority.
