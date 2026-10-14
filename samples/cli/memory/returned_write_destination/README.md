# Returned write destination

Expected exit: 0. A helper returns a write-only loan into caller storage. The
caller replaces 7 with 42 and preserves the neighboring value 19.

Run `omega run samples/cli/memory/returned_write_destination/main.omg`, or use
`--both` to compare native and Terminal execution. Both paths exit 0 on Intel
macOS. Reception transports the descriptor without reading the referent or
widening access. The composition fixture covers local moves, field and parameter
sources, runtime selection, mutable attenuation, call operands and computed
state-transition arguments. Separate controls reject observation, access
widening, hidden producer-operand reads and live-loan conflicts.
