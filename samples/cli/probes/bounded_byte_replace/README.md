# bounded_byte_replace

`&mut [u8;16] in Utf8` + `ensures output in Utf8` — successive literal stores each re-prove the domain write-side across chained states.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
