# generic_seq_map_filter

First sample with generic machine-parameter bounds:
`machine map<T, U, machine F>` where `machine
F(value: T) -> U` — `map<increment>`/`filter<positive>`
instantiate over a recursive `Seq<T>`; `terminates by
items` proves the recursive calls descend.

## Learnings

- `transition` on a generic sum instantiation rejects:
  "case membership must test a value of the exact
  declaring data type" — construct and transform
  `Seq<u64>`, but dispatch stays inside the generic
  `Seq<T>` machines.
- Payload arms rename with `as`, never `head: first`.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
