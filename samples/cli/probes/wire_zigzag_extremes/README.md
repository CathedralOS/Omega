# wire_zigzag_extremes

First sample round-tripping signed extremes through the
generated codec: `i64::MIN`/`i64::MAX` zigzag plus an
`[i32; 2]` array member — bit-exact through encode →
decode, framing proves 37 bytes each way.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
