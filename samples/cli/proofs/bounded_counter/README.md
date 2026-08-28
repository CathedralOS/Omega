# Bounded Counter

A counter that saturates at a bound. Showcases `[copy]` type
properties (frozen decision 8) and saturating arithmetic over a dispatched
self-write path driven by scalar-argument machine calls. Runs to exit **70**.

```
omega --target windows_x64 --build-dir build samples/bounded_counter/main.omg
./build/omega-program.exe   # exit 70
```

Companion to `samples/vending_machine` (the case-payload showcase). This one
deliberately drives state with scalar arguments rather than threading a
by-value case parameter into a self-writing sub-machine — that combination is
a known native miscompile tracked by
`tests/omega/pending/calls/by_value_case_param_self_write_lost`.
