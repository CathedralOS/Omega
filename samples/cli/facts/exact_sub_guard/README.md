# exact sub guard

`remaining - 2` is exact u64 because `requires remaining <= 5` and the
`remaining >= 2` transition guard prove the operand order. No domain, no
`as`. There is no ProcessExit provider. Each place is compile-checked only.

```sh
omega --check samples/cli/facts/exact_sub_guard/local/main.omg
omega --check samples/cli/facts/exact_sub_guard/field/main.omg
omega --check samples/cli/facts/exact_sub_guard/parameter/main.omg
omega --check samples/cli/facts/exact_sub_guard/call_result/main.omg
omega --check samples/cli/facts/exact_sub_guard/match_join/main.omg
omega --check samples/cli/facts/exact_sub_guard/const_name/main.omg
omega --check samples/cli/facts/exact_sub_guard/other_state/main.omg
omega --check samples/cli/facts/exact_sub_guard/other_machine/main.omg
omega --check samples/cli/facts/exact_sub_guard/target_body/main.omg
```

The `invalid` child of each place drops that operand-order proof and must
reject exact `remaining - 2`.
