# Fixed interior region byte roundtrip

A fixed interior typed region can be copied through byte views without changing its values or either neighboring word.

This is the source-footprint prerequisite of checked fixed-layout decoding:
`[1..6]` names all five words, rather than one indexed word. Both byte views
cover exactly 40 bytes; the program checks every source and destination word,
including their distinct neighboring canaries. Its reject twin changes only
the source endpoint from 6 to 5, so 32 source bytes cannot license a 40-byte view.
A second control writes the destination while its exclusive byte loan is active;
retaining complete region geometry must preserve that borrow rejection.

The layout decoder remains in `fixed_layout_byte_roundtrip`: this program does
not establish a stronger typed invariant from arbitrary bytes. This landing covers the local place; the remaining eight transport places
and other hosts remain open.
