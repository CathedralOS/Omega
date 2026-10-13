# terminal_contract

Proposition evidence vocabulary: a `proposition` may be aliased to a
parameterized form (`proposition ... = terminal_relation<T, Limit,
Generator>(value)`) and bound to an evidence trait, then cited in
requires/ensures positions — beside scalar requires/ensures machines,
`127u64 as u8` literal narrowing, and a guard-narrowed `as u8` under
`value <= 255u64`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
