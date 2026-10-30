# Callback Contract Reach

Compile-scope probe: machine-parameter constraints by named trait
requirement, and effect-carrying contracts. No standard-library
dependency.

- `pub boundary trait Sink { }` — an empty boundary trait names an
  effect with no machines.
- `trait StepContract { machine step -> u64 reaches Sink terminates; }`
  — requirements carry effect clauses.
- `machine quiet satisfies StepContract::step terminates` — an
  implementation may reach a SUBSET of the declared effects (quiet
  reaches nothing and still satisfies the `reaches Sink` requirement).
- `machine traverse<machine Step> requires machine Step satisfies
  StepContract::step` — machine param constrained by a named
  requirement; `Step(value)` calls the parameter.
- `data Buffer { bytes: [u8; length()] }` — const-eval runs the whole
  generic chain at type position (`length()` = 7).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/callback_contract_reach/main.omg
mbx run -p omega -- update --project samples/cli/probes/callback_contract_reach
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
