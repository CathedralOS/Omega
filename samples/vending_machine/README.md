# Vending Machine

An event-driven vending machine: a stream of case-payload `Event`s (coins
inserted, buttons pressed) folded into a `[copy, zero_init]` `Register`. It
exercises features from the 2026-06 waves together in one program — `case`
members with named payloads, type properties on plain data, payload binding
in transition arms, and value-position machine calls — and reports the final
state as an exit code (expected: 70).

## Status: blocked on a known native miscompile

This sample currently exits **72**, not 70. It surfaced a real backend bug,
captured minimally as `canaries/pending/calls/by_value_case_param_self_write_lost`:

> A `&mut self` machine that takes a **by-value case-bearing parameter** and
> writes `self.<field>` in a dispatched substate loses the write — the caller
> observes the pre-call value. (`Main::apply(self, event: Event)` writing
> `self.register.balance` is dropped.) The identical shape with a scalar
> argument persists the write correctly, so this is the `&mut self`
> write-back leg of the by-value aggregate-parameter family.

Once that pending canary is fixed and promoted, this sample runs to exit 70
and becomes a clean end-to-end showcase. It is committed now as the
aspirational target + a human-readable companion to the regression canary.

## Build

```
omega --target windows_x64 --build-dir build samples/vending_machine/main.omg
./build/omega-program.exe   # exit code reports the run
```
