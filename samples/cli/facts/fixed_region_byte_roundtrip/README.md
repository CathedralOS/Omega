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
Each [reject twin](invalid_call_result/main.omg) changes only the source endpoint
from 6 to 5, preserving the complete operation and runtime oracle.

All three ordinary programs agree at native/Terminal exit 0 on Mac ARM64, and
their three short-region twins reject for representation incompatibility.
The reused optimized Rust producer is `fddd05d57572`, with its matching bundled
library and unchanged compiler inputs; the frozen witness source is
`36cacd8d3083`. These observations exercise checked-to-Terminal borrow/record
transport, ARM64 argument and return placement, the complete byte assignment,
activation storage and Mach-O execution. Every source and destination word is a
runtime falsifier for an incorrect offset, truncated copy or changed canary.

The [match-join](match_join/main.omg), [const-name](const_name/main.omg),
[nested-machine](other_state/main.omg), [other-machine](other_machine/main.omg)
and [selected-target](target_body/main.omg) places carry the region's end
coordinate, 6, into `&source[1..end] as &[u8; 40]`. A window is fixed geometry,
so its endpoint must name one value: every arm of a runtime-subject match, a
named constant, a nested machine's singleton `requires`, one machine's ensured
result passed to another, and a Build-selected implementation's ensured result.
Recast checking and place lowering read that endpoint through one
`window_coordinate` routine, so a window admitted by checking is the window
lowering places. The match-join program calls its copy with both subject values.
The selected implementations return the end rather than take `&mut` storage,
because exclusive-loan arguments behind a selected implementation remain open
under **BUILD-OWNED-TARGET-SELECTION**.

Each of these twins changes only the end from 6 to 5. Four reject for
representation incompatibility; the match-join twin keeps one arm at 6, so the
joined end no longer names one region and it rejects with `recast source window
endpoint is not statically exact`. All five positives agree at native/Terminal
exit 0 on Mac ARM64 with the producer built from `2669e12a2d9` plus this repair.

All nine places now have positives and reject twins pinned under
`tests/omega/{pass,fail}/facts/`. Other-host execution beyond the observations
above, and the broader layout decoding and representation-family acceptance
under **LIBRARY-REPRESENTATION-FAMILIES**, remain open.
