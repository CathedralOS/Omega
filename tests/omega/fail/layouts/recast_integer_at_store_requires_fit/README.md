# Physical integer storage fit

The semantic `i64` field admits 128, but its signed eight-bit storage does not.
Checking must reject the assignment itself:

```sh
omega --check tests/omega/fail/layouts/recast_integer_at_store_requires_fit/main.omg
```

The expected diagnostic is `cannot prove assigned value fits integer storage`.
This must not depend on native lowering or a Terminal proof search. The same
assignment rule applies to an ordinary field and a field reached through a
recast reference. The positive control is
`tests/omega/pass/layouts/runtime_plan_laid_integer_at_proved_write_exit/main.omg`:
declaration and flow bounds permit its stores, and `omega run --both` returns 72.
