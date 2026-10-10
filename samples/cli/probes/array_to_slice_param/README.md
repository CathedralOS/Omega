# array_to_slice_param

First sample passing a sized-array borrow to a slice
parameter: `count(&main.buf)` coerces `&[u8; 8]` →
`&[u8]` implicitly at the call site, for field arrays
and locals alike.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
