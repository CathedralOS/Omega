# dependent_subtract

The relational subtraction rule: `count - used` under `used <= count` proves
non-negative in exact `u32`. A `data Main where count <= 11` premise keeps
that fact standing, so the difference also fits exact `as i32`.

Compile-checked only: no standard-library dependency and no `Main::exit`.
Check each place; do not run it.

```
omega --check samples/cli/facts/dependent_subtract/<place>/main.omg
```

The places are `local`, `field`, `parameter`, `call_result`, `match_join`,
`const_name`, `other_state`, `other_machine`, and `target_body`. Each
`invalid/` drops the order and must reject with exact `u32` overflow.
Values stay unknown: a literal assignment would make the difference succeed
by folding and would not witness the relation.
