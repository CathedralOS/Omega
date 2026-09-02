# State-machine Delta compiler experiment

`delta_compiler.gamma` is the latest speculative Gamma implementation of the
typed state-machine Delta slice. It covers nominal sums, records, fixed arrays,
typed machine variables, states, exhaustive transitions, calls, and direct
Alpha emission. V2 adds sequential byte input/output, typed copy, and checked
dynamic array reads/writes. Its nested-parser customer builds indexed declaration
and scope arenas, permits nested shadowing, rejects same-scope duplicates with
source offsets, and reports fixed-capacity exhaustion.

The source is intentionally retained near the future canonical edge, but remains
noncanonical. Its executable customer, rejection twins, exact measurements, and
native/interpreted agreement gate live under
[`../../../../../tests/delta/state-machine-experiment/`](../../../../../tests/delta/state-machine-experiment/).
