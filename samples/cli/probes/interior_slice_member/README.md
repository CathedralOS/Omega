# interior_slice_member

A borrowed scalar slice member carries its runtime element count through
wire encode: `TelemetrySample { readings: main.source[1..4] }` takes the
interior subslice at the member value without an `&`, then
`Telemetry::encode(&sample, &mut buffer, &mut written)` frames it
length-delimited.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
