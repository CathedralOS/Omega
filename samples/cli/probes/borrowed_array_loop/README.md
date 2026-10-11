# borrowed_array_loop

A borrowed fixed-array parameter carries across re-entry edges:
`out: &mut [u8; 17]` forwards through the fill/write_digit loop and
each element stores through the borrow — a bounded-view carrier; the
referent stays caller-owned.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
