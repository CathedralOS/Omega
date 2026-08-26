# Omega bootstrap checked IR schema major 13

[`CKIR12`](OMEGA_BOOTSTRAP_CHECKED_IR_V12.md)

CKIR13 is the private full-width `u32 in Trapping` subtraction carrier.  It
inherits CKIR11's framing, table order, row widths, ordering, visibility,
resources, and opcodes, and adds opcode 26.  Earlier CKIR4 through CKIR12
identities and meanings remain frozen.  CKIR13 does not compose CKIR12's
static-view type or opcodes 22 through 25 in the same carrier.

## Full-width semantic scalar words

In CKIR13, type range endpoints and scalar `Const` immediate 0 are exact
little-endian 32-bit semantic words.  Canonical unconstrained kind-2 `u32` has
low `0` and high `0xffffffff`; canonical `u32 in Trapping` is that row with
the inherited Trapping domain flag.  A Delta checker may store these bits in a
signed `i32` but must decode them positionally and preserve all 32 bits.

Structural IDs, counts, offsets, ordinals, child spans, and `NO_ID` retain the
inherited structural decoder and bounds.  Aggregate constant scalar nodes
remain limited to the earlier nonnegative signed range in this milestone;
full-width scalars are admitted as direct opcode-1 constants.

## Opcode 26: `Subtract`

Opcode 26 has exactly two visible operands, zero flags and immediates, and one
dense result.  Both operands and the result have the same exact canonical full
`u32 in Trapping` type.  The mathematical subtraction is evaluated over
`0..=0xffffffff`; a negative result traps before any result is published.
Otherwise the exact `u32` result is published.  A CKIR13 carrier contains at
least one opcode-26 row.

Malformed schema/type/scalar/operation/visibility relations select status 251
without output.  Inherited operation and operand ceilings select 252.  The
independent CKIR13 reference validates and interprets maximum, near-maximum,
ordinary, equal, and underflow cases without using the backend.
