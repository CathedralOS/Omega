# Descent measures

Depend-free compile-scope probe: termination measures and inductive
ensures as user code — the first sample carrying `terminates by` outside a
shipped module.

- `countdown` recurses on a scalar measure (`terminates by n`) and carries
  a self-referential ensures — the recursive arm assumes the machine's own
  guarantee at `(n - 1, acc + n)`, so the checker proves
  `2 * countdown(n, acc) == 2*acc + n*(n+1)` inductively.
- `climb` recurses UPWARD on a two-argument distance measure
  (`terminates by (index, limit) -> Nat::BoundedDistance`) behind a
  comma-separated requires roster; its ensures is an integer inequality
  over `embed`, which neither Wrapping nor Saturating could stand in for.

The entry accepts only `countdown(4, 0) == 10` and `climb(4, 0, 0) == 4`.
Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/descent_measures/main.omg
mbx run -p omega -- update --project samples/cli/probes/descent_measures
```

Both complete green on Linux x86-64; `omega.lock` publishes locally and is
ignored like every sample lock.
