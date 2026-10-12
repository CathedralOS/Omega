# Ranked record and independent snapshot

The live `pair.left` measure decreases around the cycle, while `copy` remains
an independent snapshot of the original input. The finishing state is outside
the cyclic component and may return that snapshot without retaining the rank
formal. The executable entry now calls the original walk with five and traps
unless its result is still five. Native and Terminal execution must exit zero.

```sh
omega run --both tests/omega/pass/termination/rank_range_record_snapshot_compile/main.omg
python3 tools/corpus_gate.py --both --filter termination/rank_range_record_snapshot_compile,termination/rank_arrival_captured_record_fields_exit,termination/rank_arrival_same_carrier_fields_exit
python3 tools/corpus_gate.py --filter termination/rank_arrival_unrelated_field_step
```

The public command uses ordinary local package/entry review. No broad acceptance
policy is included. Matching-host execution here is qualified on macOS ARM64;
declaring other targets is not evidence of executing them.

The four original `u32 [0..=5]` declarations at `568c41c7a7f` are now ordinary
propositions: `Pair.left` uses data `where left <= 5`; `walk.remaining`, `s.copy`
and `finish.result` use arrival `requires` with their own parameter `<= 5`.
The unsigned carrier supplies each zero floor. No result suffix existed on
`walk`; its rank range remains a separate witness, not a type or allocation.

The neighboring direct and captured-record programs decrement the selected
left field and return zero. The invalid sibling decrements only the unrelated
right field and must still reject strict rank decrease. The same-carrier fields
have distinct declaration identities despite equal bounds.

A mechanical probe carries the snapshot through a helper's `ensures result ==
value`, a local, and a renamed finishing formal; both engines still return the
authored result. Supplying six instead refuses `walk`'s requirement. These probes
are restored to the original walk and valid five input before landing.
