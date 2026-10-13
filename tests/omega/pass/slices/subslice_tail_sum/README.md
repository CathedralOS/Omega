# subslice_tail_sum

A tail-recursive value-returning machine whose accumulator argument
READS a slice element of the SAME slice it subslices:
`sum(s[1..], acc + s[0])` — the element `s[0]` must be read before the
`s` parameter's descriptor retargets to `s[1..]`. Ranked by
`terminates by s -> Slice::Length`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
