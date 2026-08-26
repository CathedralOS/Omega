# Omega-bootstrap normalized resolution handoff, schema major 5

[`OMGRSW4`](OMEGA_BOOTSTRAP_RESOLUTION_V4.md)

`OMGRSW5` is the bounded resolution successor for exact full-width `u32 in
Trapping` subtraction custody.  It preserves every earlier declaration,
ordering, source-span, and resource rule except where this contract explicitly
widens semantic scalar words.

The magic is `OMGRSW5\0`, schema major is 5, schema minor and flags are zero,
and the inherited header size is 84.  Tables, row widths, checked offsets,
524,288-byte ceiling, exact EOF, status 251 malformed relation, and status 252
resource relation are unchanged.

## Canonical selection and scalar representation

Any body subtraction token selects OMGRSW5, even when both written operands
are small.  Any decimal token above 2,147,483,647 also selects OMGRSW5.
Values through 4,294,967,295 are accepted; 4,294,967,296 rejects with status
251 and no output.  A source requiring neither relation retains its least
byte-identical OMGRSW1 through OMGRSW4 witness.

Semantic `u32` words are encoded as their exact little-endian 32-bit bit
pattern.  A Delta implementation may hold that pattern in a signed `i32`, but
must not interpret a negative host value as a missing ID or truncate it to a
signed range.  Structural IDs, counts, extents, offsets, array lengths, and the
inherited `NO_ID` sentinel remain structural words with their earlier bounds.
Consumers therefore decode semantic scalar positions separately.

Every canonical unconstrained `u32` type row, including `u32 in Trapping`, has
range `0..=0xffffffff`.  Explicit source range bounds remain limited to
`0..=0x7fffffff` in this bounded resolver; OMGRSW5 does not add full-width
authored range syntax.  The resolver records whether a range was authored, so
an explicit `0..=2147483647` is not canonicalized into bare full `u32`.

## Evidence and non-expansion

The focused OMGRSW5/OMGLOWE producer gate checks native/self agreement, exact
maximum-literal custody, unconditional V5 selection for subtraction, overflow
rejection, full canonical `u32 in Trapping`, and least-version preservation.

OMGRSW5 does not resolve the meaning of subtraction, choose overflow policy,
admit nested arithmetic, or define CKIR/backend behavior.  Those relations are
owned by the exact lowering and CKIR13 contracts.
