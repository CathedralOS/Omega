# slice_tail_rank

`terminates by entries;` — the default slice-length order ranks the
tail recursion `walk(main, entries[1..])` over `&[Entry]` views.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
