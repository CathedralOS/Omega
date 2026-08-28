# Vending Machine

An event-driven vending machine: a stream of case-payload `Event`s (coins
inserted, buttons pressed) folded into a `[copy]` `Register`. It
exercises features from the 2026-06 waves together in one program — `case`
members with named payloads, type properties on plain data, payload binding
in transition arms, and value-position machine calls — and reports the final
state as an exit code (expected: 70).

## Status: working — exits 70

The native miscompile that previously caused this to exit 72 has been fixed
(2026-06-12). The bug was in `InlineBranching` argument materialization: a
`StructLiteral` argument (`Event::Insert { cents: 50 }`) was never written
into the callee's parameter slot, so the case tag stayed 0 (Idle), the
dispatch guard failed, and `self.register.balance` was never updated.

The fix and a minimal regression canary are in
`tests/omega/pass/calls/by_value_case_param_self_write_exit`.

## Build

```
omega --target windows_x64 --build-dir build samples/vending_machine/main.omg
./build/omega-program.exe   # exit code reports the run
```
