# entry_param_alias

A machine parameter aliases caller storage: `alias_write(health: &mut i32)`
assigns `health = 1` through the alias, landing in the caller's
`main.hits`.

## Gaps found while authoring (filed)

- A bare `&mut i32` entry context is refused at bind time — "entry machine
  `Main::main` context `health` has no exact data declaration". Corpus
  checks the same signature without a build binding, so the refusal is a
  binding-side rule, not a signature rule.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
