# Higher Order Streams

Depend-free compile-scope probe: machines as type parameters — the first
sample with `data Stream<machine S>` behind a `where machine` signature
bound, plus a recursive enum and higher-order schema forwarding.

- `Index` is a recursive enum (`Next` carries an `Index` payload), giving
  a finite chain observable depth.
- `Stream<machine S>` binds a static machine parameter under
  `where machine S(index: Index) -> Index`; `Stream::More` stores
  `tail: Stream<S>` — recursion through the machine-typed parameter.
- `forward_schema<identity_schema, succ>` passes one machine through
  another machine's parameter — static dispatch with no `dyn` selector.

The machine builds a two-element `Stream<succ>`, forwards it through
`identity_schema`, destructures the result twice, and accepts on the
`Empty` tail. Compile-checked only (no std dependency):

```sh
mbx run -p omega -- --check samples/cli/probes/higher_order_streams/main.omg
mbx run -p omega -- update --project samples/cli/probes/higher_order_streams
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
