# indexed_receiver_element_arg

Indexed element of an immutable receiver field loans into a `&T` argument:
`Main::probe(main: &Main, i) requires i < 4` passes
`&main.list.items[i]` to `Elem::clone_of`; `List::get(&main.list, i)`
forwards the same shape one call deeper.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
