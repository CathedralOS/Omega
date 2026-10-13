# indexed_state_arg

`items[index]` as a composite state argument on a guarded edge — the indexed element evaluates only on the selected arm; unselected edges never evaluate out-of-bounds copies.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
