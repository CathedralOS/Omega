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

- `filter<positive>` admits `positive(value: &i32)` against
  `Predicate(value: &T)` once the call's tuple substitutes `T := i32`,
  including inside the specialized recursive body, whose `tail` payload
  denotes the receiver's substituted `Seq<i32>`.

Compile-checked only: no standard-library dependency and no
`Main::exit`. `Seq<T>` is recursive by value, so it has no finite runtime
representation and stays proof-only ("Recursive mathematical data without a
finite runtime representation remain proof-only",
`wiki/spec/proofs/contracts.md`); `build.omg` binds no program entry.
