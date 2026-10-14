# composite_state_args

Composite values as internal state arguments: record literal, call
result, and local each arrive in a fresh owned state parameter; the
guarded edge's `make` call runs exactly once.

## Learnings

- Arrival arguments evaluate left to right
  ([evaluation schedule](../../../../../wiki/spec/language/expressions.md#evaluation-schedule)),
  and exclusive loans must not overlap
  ([borrows and aliases](../../../../../wiki/spec/language/ownership.md#borrows-and-aliases)).
  `take_call(main, make(&mut main.counter, 5), local)` forwards `main` and
  then borrows `main.counter` while that reference is held, so it rejects
  (`fail/borrows/state_argument_borrows_forwarded_reference`); the call
  result goes first instead.

- States see no enclosing locals and no `main` unless declared as state
  params — chain `(main: &mut Main, ...)` through every state.
- `i32` arithmetic on record fields needs explicit wrapping:
  `((x as i32 in Wrapping) + (1 as i32 in Wrapping)) as i32` — wrap every
  operand, then `as i32` strips the domain back for an `i32` field.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
