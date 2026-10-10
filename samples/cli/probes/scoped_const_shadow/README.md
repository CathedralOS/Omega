# scoped const shadow

Compile-scope probe. No standard-library dependency.

- suspend_route: `pub boundary data Sleeper`, `boundary machine
  Sleeper::park suspends;`, `suspend Sleeper::park(7)` inside a
  `suspends;` machine, `Token in AcrossSuspend` domain param.
- crash_guard_face: `crashes Trap <face>` — member and scalar
  crash-route guard faces between the route decl and the body.
- scoped_const_shadow: `const Sizes::SIZE: Index` scoped const +
  record-typed const param + `let Sizes: u64` local shadow.
- slice_reborrow: `&mut buf` reborrow of a `&mut [u64]` param
  into a second machine.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/scoped_const_shadow/main.omg
mbx run -p omega -- update --project samples/cli/probes/scoped_const_shadow
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
