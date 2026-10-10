# operand destination mix

Compile-scope probe. No standard-library dependency.

- match_param_moves: `match selected { true -> left, false -> right
  }` moves owned params; literal-arm match on u64 with
  `_ -> fallback` local.
- operand_destination_mix: anonymous comptime arithmetic folds at
  every destination — `(2^64 / 2^64) | 8`, match-arm fractions
  `7/2*2 == 7`, `as u64` on match results. 2^64 literals are
  legal (unbounded comptime).
- named_proposition_roster: `proposition first_ready() evidence
  Evidence` (evidence = a TRAIT), named `requires first:
  first_ready()`/`ensures first_out: ...` rosters, satisfies impl
  wiring `first_out = local_first`.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/operand_destination_mix/main.omg
mbx run -p omega -- update --project samples/cli/probes/operand_destination_mix
```
