# State-machine Delta compiler experiment

`delta_compiler.gamma` is the latest speculative Gamma implementation of the
typed state-machine Delta slice. It covers nominal sums, records, fixed arrays,
typed machine variables, states, exhaustive transitions, calls, and direct
Alpha emission. V2 adds sequential byte input/output, typed copy, and checked
dynamic array reads/writes. Its nested-parser customer builds indexed declaration
and scope arenas, permits nested shadowing, rejects same-scope duplicates with
source offsets, and reports fixed-capacity exhaustion.

The latest experiment admits arrays of any exactly one-word type. A recursive
syntax customer uses a nominal node-kind sum over parallel indexed arenas,
performs an explicit postorder fold and branch-selection rewrite, and serializes
the transformed tree without recursion or heap allocation.

A second Epsilon-shaped customer lays out and encodes all 21 Alpha opcodes from
22 nominal symbolic item variants. Full-profile fixed arenas admit 1,048,572
items and labels; exact and adjacent payload-limit cases pin the selected Alpha
maximum. It resolves forward labels, rejects malformed or incomplete programs
before output, and emits exact target bytes. Supporting its label arithmetic
adds typed multiplication, division, and signed less-than branching to the compiler.

The source is intentionally retained near the future canonical edge, but remains
noncanonical. Its executable customer, rejection twins, exact measurements, and
native/interpreted agreement gate live under
[`../../../../../tests/delta/state-machine-experiment/`](../../../../../tests/delta/state-machine-experiment/).
