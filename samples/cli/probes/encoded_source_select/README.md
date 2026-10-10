# encoded_source_select

`machine select_recursive<'storage>(first: &'storage mut i64, second:
&'storage mut i64, ...) -> &'storage mut i64` — a named `'storage`
lifetime carries `&mut` params to the `&mut` result, and the declared
source union picks between the encoded leaf and the ordinary scalar.
`requires remaining <= 4` + `terminates by remaining -> Nat::Descending
in 0..=4;` ranks the tail-recursive call; `false -> value` returns a
param reference straight from the arm; `&mut view.value.value` loans the
plan-laid leaf as a plain `&mut i64` through `retain`/`write_returned`.

## Learnings

- The declared source union forbids touching either candidate while one
  borrow is live — snapshot each result (`let minimum: i64 = encoded`)
  to end the borrow before the next `select_recursive` call.
- `&'storage mut` + `-> &'storage mut` is the spelling for
  param-through-to-result reference flows.

Compile-checked only: `use omega::language::core::layout` is depend-free.
