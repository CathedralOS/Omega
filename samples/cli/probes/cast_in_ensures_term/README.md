# cast_in_ensures_term

First sample with a cast inside a contract term: trait
`ExactInteger::form` declares `ensures (value as u8 in
Wrapping) == 3u8` — an abstract prior fact the caller
cites — alongside a concrete sibling doing the same
f64→u8 narrowing operationally via `u8 in Wrapping`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

## Gaps found while authoring (filed)

- `value as u8` (unqualified f64→u8 cast) cannot be
  proved even under `requires value == 3.0` — the checker
  reports "cannot prove unqualified `U8::from_f64`
  operand is finite and truncates into u8" and suggests a
  declared range, dominating guard, or `u8 in
  Trapping`/`Saturating` result. The abstract ensures
  clause is the only route that claims the fact; a
  `satisfies` impl cannot discharge it operationally.
