# write_only_state_forward

`&write` write-only loans forward through nested machines whole-param: `forward(&write dst, v)` → `machine write_value(destination: &write i32, ...)` → `replace(&write destination, v)` — the qualifier rides every hop including the state-arg loan `&write destination`.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
