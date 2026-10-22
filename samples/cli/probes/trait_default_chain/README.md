# Trait Default Chain

This probe retains legacy source awaiting `STATIC-MACHINE-MIGRATION` and
`TRAIT-IMPLEMENTATION-SOURCE-MIGRATION` in [TASKS.md](../../../../TASKS.md).
Data-owned machines and reference rows are retired; replace them with ordinary
free/module calls inside inline named-implementation members while retaining
the behavior below. Historical green observations are not current acceptance.

Depend-free compile-scope probe: trait inheritance with default bodies —
the first sample carrying a parent-trait bound and named implementation
bindings.

- `trait Resettable<T>: Settable<T>` gives `prepare`/`reset` default bodies
  that call the inherited `set` requirement by bare spelling in the
  trait's scope.
- `CounterResettable satisfies Resettable<Counter>` binds the requirement
  with `Settable::set = Counter::set`, a data-attached machine — no
  conformance subject or implicit self.
- `Store<Container, Item>` is a two-parameter trait whose `put_twice`
  default calls `put` under the conformance's `i32`.

The machine resets a counter (value 30, `prepared` set by the bound row's
10-branch) and `put_twice`-appends 7 to a wrapping ledger (count 2,
sum 14). Compile-checked only (no std dependency):

```sh
mbx run -p omega -- --check samples/cli/probes/trait_default_chain/main.omg
mbx run -p omega -- update --project samples/cli/probes/trait_default_chain
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
