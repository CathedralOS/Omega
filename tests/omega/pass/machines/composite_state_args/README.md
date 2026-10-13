# composite_state_args

Composite values as internal state arguments: record literal, call
result, and local each arrive in a fresh owned state parameter; the
guarded edge's `make` call runs exactly once.

## Learnings

- States see no enclosing locals and no `main` unless declared as state
  params — chain `(main: &mut Main, ...)` through every state.
- `i32` arithmetic on record fields needs explicit wrapping:
  `((x as i32 in Wrapping) + (1 as i32 in Wrapping)) as i32` — wrap every
  operand, then `as i32` strips the domain back for an `i32` field.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
