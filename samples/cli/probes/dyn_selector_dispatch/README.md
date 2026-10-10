# Dyn Selector Dispatch

Depend-free compile-scope probe: runtime implementation selection — the
first sample passing `Impl as dyn Trait<Data>` through an ordinary
parameter.

- `pick` receives `shape: dyn Shape<Item>` and runs `Shape::code` through
  whichever named implementation the call site selected; the selector
  carries no data of its own.
- `CircleShape` reports 9 and `SquareShape` reads the item's `sides`, so
  `9 * 10 + 4 == 94` proves both selections reach their own rows.

The machine is compile-checked only (no std dependency, no ProcessExit
provider):

```sh
mbx run -p omega -- --check samples/cli/probes/dyn_selector_dispatch/main.omg
mbx run -p omega -- update --project samples/cli/probes/dyn_selector_dispatch
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
