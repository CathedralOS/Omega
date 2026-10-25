# Rank Range Cycle

Compile-scope probe: rank-range termination measures across a
two-machine descent cycle. No standard-library dependency.

- `terminates by remaining in 0..=9` bounds the rank; the matching
  `requires remaining <= 9` roster brings the bound into the body.
- `machine hold(...) requires pending <= 9` — a subordinate state's
  requires participates in the cycle: `hold -> step(pending)` is a
  weak edge, the strict `step -> count(n - 1)` edge completes every
  cycle's descent.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/rank_range_cycle/main.omg
mbx run -p omega -- update --project samples/cli/probes/rank_range_cycle
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
