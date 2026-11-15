# Bounded Counter

A counter that saturates at a bound. Showcases `[copy]` type
properties (frozen decision 8) and saturating arithmetic over a dispatched
self-write path driven by scalar-argument machine calls. Runs to exit **70**.

```
omega --target windows_x86_64 --build-dir build samples/cli/proofs/bounded_counter/main.omg
./build/omega-program.exe   # exit 70
```

Companion to `samples/cli/games/vending_machine` (the case-payload showcase).
