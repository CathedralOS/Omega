# Policy Array Modes

Compile-scope probe: collection policy + crash-route machines. No
standard-library dependency.

- `modes: [Mode; 4] in Trapping` — field policy governs indexing;
  `index: u64 in Trapping` params carry the arithmetic contract.
- `Table::weigh crashes Trap` guards `index < table.count` before
  element dispatch; `as u64` casts the call result.
- `let mut table` deferred init; `machine Main::main` entry.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/policy_array_modes/main.omg
mbx run -p omega -- update --project samples/cli/probes/policy_array_modes
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
