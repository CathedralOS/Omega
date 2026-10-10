# sibling_receiver_slice_arg

First sample passing two disjoint `&mut` loans from
sibling fields of one owner: `Sink::fill(&mut main.sink,
&mut main.buf[..])` — receiver and whole-array `[..]`
byte slice coexist for the call.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
