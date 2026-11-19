# cyclic_shared_array

First sample taking a shared borrow of a plain fixed
array through a state cycle: `scan(numbers: &[i32; 3])`
loops step ↔ body under the unranked-cycle eligibility
gate — the shared loan freezes its referent so every
arrival observes the same storage; `body`'s `index < 3`
requirement makes `index + 1` exact inline.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
