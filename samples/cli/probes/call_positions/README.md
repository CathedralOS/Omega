# call positions

Compile-scope probe. No standard-library dependency.

- `twelve() as i64`, `twelve() == 12`, `0 - twelve()`,
  `Main::add(main, twelve(), twelve())`, `main.total = twelve() * 2`,
  `transition twelve() + 1 == 13` — calls valid in every operand
  position.
- `Main::tick(main) * 10 + Main::tick(main)` ordered single
  evaluation per operand; `false && Main::tick(main)` short-circuits
  (tick count stays 2).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/call_positions/main.omg
mbx run -p omega -- update --project samples/cli/probes/call_positions
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
