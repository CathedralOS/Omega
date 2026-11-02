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
not establish a stronger typed invariant from arbitrary bytes. The local place
has Linux x86_64 observations.

The [field](field/main.omg), [parameter](parameter/main.omg), and
[call-result](call_result/main.omg) places carry the same source byte view in a
`ByteView` record. The exact five-word footprint and backing survive field
projection, a call argument, and a returned descriptor; the consumer still copies
all 40 bytes and checks all fourteen words, including the distinct canaries.
Each [reject twin](call_result/invalid/main.omg) changes only the source endpoint
from 6 to 5, preserving the complete operation and runtime oracle.

All three ordinary programs agree at native/Terminal exit 0 on Mac ARM64, and
their three short-region twins reject for representation incompatibility.
The reused optimized Rust producer is `fddd05d57572`, with its matching bundled
library and unchanged compiler inputs; the frozen witness source is
`36cacd8d3083`. These observations exercise checked-to-Terminal borrow/record
transport, ARM64 argument and return placement, the complete byte assignment,
activation storage and Mach-O execution. Every source and destination word is a
runtime falsifier for an incorrect offset, truncated copy or changed canary.

The other-machine and selected-target places remain open, together with
remaining host observations, product-producer support and broader layout
decoding and representation-family acceptance.

The [match-join](match_join/main.omg), [const-name](const_name/main.omg), and
[nested-state](other_state/main.omg) places have matching Linux x86_64
observations in [run 41484907405](https://github.com/CathedralOS/Omega/actions/runs/41484907405),
with optimized Rust producer `eab17336ac15d1e8a9d68d4e44fe9bb29821aafd`.
The join executes both arms and checks each selected marker; the constant owns
the copied seven-word source; the nested state receives the stored byte view
and exclusive output, performs the copy, and returns before the original arrays
are checked. Every program still verifies all fourteen words.

All six source outcomes, three native/Terminal exit-0 agreements and six ordinary
CLI sample outcomes meet their original oracles. Each invalid program changes
only the source endpoint from 6 to 5 and rejects for representation incompatibility.
Producer preparation took 309.444 seconds, source checking 23.102 seconds,
canonical execution 17.665 seconds and ordinary sample observation 40.513 seconds.
No earlier transport cohort or broader corpus was repeated.
