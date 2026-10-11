# domain_subslice_carry

Primitive byte domains survive runtime subslices:
`bytes[0..main.end]` stays `in Ascii` through a helper parameter and a
named local, and the empty view `prefix[0..0]` remains a valid ASCII
view — even when `prefix` was produced inside a machine that never
saw the original range.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
