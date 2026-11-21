# sibling_receiver_slice_arg

First sample passing two disjoint `&mutable` loans from
sibling fields of one owner: `Sink::fill(&mutable main.sink,
&mutable main.buf[..])` — receiver and whole-array `[..]`
byte slice coexist for the call.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
