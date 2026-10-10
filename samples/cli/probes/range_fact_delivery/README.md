# range_fact_delivery

Declared-range delivery proven at every site: literal `admit(0.5)`, narrower
declared param `forward(value [0.0..=0.25])` delivered into `[0.0..=0.5]`,
nested call `outer(admit(bounded))` inside an expression, and guard fact
`index < 4` discharging `count(index)` whose param is `u64 [0..=4]`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
