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
reject exact `remaining - 2`. Each one keeps the place's program shape and
removes only the `remaining >= 2` proof, so its diagnostic is the exact
arithmetic overflow above. Restoring only the lower bound makes each twin
check, which is why the twin breaks this fact and nothing else.

```sh
omega --check samples/cli/facts/exact_sub_guard/local/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/field/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/parameter/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/call_result/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/match_join/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/const_name/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/other_state/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/other_machine/invalid/main.omg
omega --check samples/cli/facts/exact_sub_guard/target_body/invalid/main.omg
```

Each command exits 1. `python3 tools/sample_gate.py --filter exact_sub_guard`
checks all 18 projects and reports 9 intended rejections. The same fact and its
twin are pinned as fixtures under
`tests/omega/pass/facts/exact_sub_guard_through_<place>_compile` and
`tests/omega/fail/facts/exact_sub_guard_through_<place>_rejected`.
