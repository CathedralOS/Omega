# Conservative CKIR13 subtraction backend implementation note

[`CKIR13`](OMEGA_BOOTSTRAP_CHECKED_IR_V13.md)

The shared checked-IR backend accepts schema major 13 without changing older
carrier paths.  It independently requires the exact two-operand canonical
full-`u32 in Trapping` relation and at least one opcode 26.

On x86-64, opcode 26 loads the left 32-bit value, performs a 32-bit `sub` from
the right frame value, branches on carry (`jb`) to the shared `ud2` trap, then
runs the ordinary unsigned low/high checks and stores the result.  Carry is the
exact unsigned-underflow predicate; no signed comparison or signed-D0 scalar
truncation participates.  Direct full-width constants use the same exact
32-bit scalar-word decoder and emitter.

The focused backend gate checks native/self artifact identity, the exact
load/subtract/carry/range/store template, independent meaning for maximum,
near-maximum, ordinary and equal operands, runtime underflow, schema/type/
arity/immediate/visibility mutations, and inherited resource ceilings.
Reference-model status 251 represents a semantic trap for the host gate; the
emitted artifact traps with `ud2` and does not define status 251 as a target
process ABI.
