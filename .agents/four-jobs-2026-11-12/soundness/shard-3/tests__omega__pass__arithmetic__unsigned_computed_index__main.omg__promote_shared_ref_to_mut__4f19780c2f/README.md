# unsigned_computed_index

`main.arr[main.wh + main.wn]` — unsigned computed index discharges its lower bound by type alone (u64 cannot be negative); the upper bound is the authored guard.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
