# Mutual ranking with reordered endpoints

The original component at `a1544e1a2ff` is unchanged: `first` preserves the
rank when calling `second`, and `second` decrements it before returning to
`first`. Their endpoint parameters have different orders. Every complete
cycle descends, and the full countdown returns its lower bound, two.
The new hosted driver traps on any other result; native and Terminal execution
must both exit zero.

```sh
omega run --both tests/omega/pass/termination/rank_range_call_component/main.omg
python3 tools/corpus_gate.py --both --filter termination/rank_range_call_component,termination/runtime_joint_rank_computed_copy_exit
python3 tools/corpus_gate.py --filter termination/rank_range_call_component_prefix_write,termination/rank_range_declared_step_unbounded
```

The public command uses ordinary package/entry review; no acceptance policy
is included. Matching-host execution here is qualified on macOS ARM64.
The other Build bindings do not claim execution on those hosts.

## Historical scalar declarations

The paired invalid program has four `u64 [0..=4]` parameters at
`3e87e79438f`: `Main::scan_a.index`, `Main::scan_a.limit`,
`Main::scan_b.index`, and `Main::scan_b.limit`. Each machine retains the
plain carriers and `requires index <= 4 && limit <= 4 && index <= limit`.
Unsigned carriers supply the zero floors. The valid reordered-endpoint
component had no scalar suffixes at its original revision.

`scan_a` still calls `bump(&mut index)`, which resets its rank subject to zero.
Its post-write guard explicitly re-establishes `limit <= 4`; together with
`index < limit`, this establishes the recursive call's scalar requirements.
It does not restore entry-relative descent: the resulting component can
repeat indices one and two forever. The control must reject the ranking
invalidation, not merely an unproved call requirement. The ordinary rank
window remains witness syntax, not a scalar type or storage capacity.

A mechanical execution probe carries the endpoints through record fields,
a helper's exact result guarantee and a local, and forwards the weak edge's
subject through `remaining + 0`. Both engines retain the result two. The
probe is restored before landing.
