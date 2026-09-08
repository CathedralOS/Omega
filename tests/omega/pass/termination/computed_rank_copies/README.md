# Computed rank copies

The named state receives two equal copies of the authored rank and uses both
in its computed arrival. The compiler must prove the copies equal on every
incoming edge, then check the rank range independently of mapping discovery.

From the repository root:

```text
cargo run -p omega -- --check --target macos_arm64 tests/omega/pass/termination/computed_rank_copies/main.omg
```

This must succeed at source checking. The paired
[`unequal_computed_rank_copies`](../../../fail/termination/unequal_computed_rank_copies/main.omg)
must reject even though each incoming value fits its declared range.
The
[`explicit_self_without_descent`](../../../fail/termination/explicit_self_without_descent/main.omg)
case must reject its nondecreasing self loop. These checks do not establish
Terminal certificate reconstruction or native execution for the state graphs.
