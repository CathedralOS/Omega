# Contract Fact Chains

Compile-scope probe: requires/ensures pairs on borrowed parameters and
named-result contracts that forward through tail calls. No
standard-library dependency.

- `arm` carries `requires start < 8` and a two-clause ensures list
  (`remaining > 0; remaining - 1 < 8`); `start + 1` establishes both.
- `tick` requires both facts — `remaining - 1` is provable because
  `remaining > 0` rules out wraparound.
- `answer`/`forward` carry `ensures result == 70` — a named-result
  contract; `forward`'s tail `transition { _ -> answer() }` forwards
  the callee's result obligation.
- `crashes Trap` declares the failure route.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/contract_fact_chains/main.omg
mbx run -p omega -- update --project samples/cli/probes/contract_fact_chains
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
